use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

use crate::ddragon::DdState;

#[derive(Serialize)]
#[serde(tag = "state")]
pub enum ConnectionStatus {
    #[serde(rename = "notRunning")]
    NotRunning,
    #[serde(rename = "notLoggedIn")]
    NotLoggedIn,
    #[serde(rename = "connected")]
    Connected {
        #[serde(rename = "summonerName")]
        summoner_name: String,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinDto {
    pub skin_id: i64,
    pub name: String,
    pub champion_id: i64,
    pub champion_name: String,
    pub rp: Option<i64>,
    /// "store" (live catalog), "estimate" (rarity-based), "none"
    pub value_source: String,
    pub rarity: String,
    pub is_legacy: bool,
    /// ISO 8601, from the store catalog. This is when the item went *on sale*,
    /// not when the skin shipped. Absent for skins pulled from the store.
    pub availability_date: Option<String>,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Minimal LCU REST client based on the client's lockfile.
///
/// The usual approach (reading LeagueClientUx.exe's command-line args) fails
/// when the client runs elevated, so we read `{install}/lockfile` instead:
/// `name:pid:port:password:protocol`.
pub struct Lcu {
    base: String,
    password: String,
    http: reqwest::Client,
}

fn lockfile_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    // Non-default installs: Riot's metadata yaml records the install dir.
    if let Ok(yaml) = std::fs::read_to_string(
        r"C:\ProgramData\Riot Games\Metadata\league_of_legends.live\league_of_legends.live.product_settings.yaml",
    ) {
        for line in yaml.lines() {
            if let Some(path) = line.trim().strip_prefix("product_install_full_path:") {
                let path = path.trim().trim_matches('"');
                candidates.push(PathBuf::from(path).join("lockfile"));
            }
        }
    }
    candidates.push(PathBuf::from(r"C:\Riot Games\League of Legends\lockfile"));
    candidates
}

pub fn connect() -> Result<Lcu, String> {
    let path = lockfile_candidates()
        .into_iter()
        .find(|p| p.exists())
        .ok_or("League client lockfile not found")?;
    let content = std::fs::read_to_string(&path).map_err(err)?;
    let parts: Vec<&str> = content.trim().split(':').collect();
    let [_, _, port, password, protocol] = parts[..] else {
        return Err("Unexpected lockfile format".to_string());
    };
    let http = reqwest::Client::builder()
        // The LCU serves a self-signed Riot cert on 127.0.0.1.
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(err)?;
    Ok(Lcu {
        base: format!("{protocol}://127.0.0.1:{port}"),
        password: password.to_string(),
        http,
    })
}

impl Lcu {
    pub async fn get(&self, path: &str) -> Result<Value, String> {
        self.http
            .get(format!("{}{}", self.base, path))
            .basic_auth("riot", Some(&self.password))
            .send()
            .await
            .map_err(err)?
            .error_for_status()
            .map_err(err)?
            .json()
            .await
            .map_err(err)
    }
}

pub async fn status() -> ConnectionStatus {
    let Ok(lcu) = connect() else {
        return ConnectionStatus::NotRunning;
    };
    match lcu.get("/lol-summoner/v1/current-summoner").await {
        Ok(v) => {
            let name = v
                .get("gameName")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .or_else(|| v.get("displayName").and_then(Value::as_str))
                .unwrap_or_default();
            if name.is_empty() && v.get("summonerId").is_none() {
                ConnectionStatus::NotLoggedIn
            } else {
                ConnectionStatus::Connected {
                    summoner_name: name.to_string(),
                }
            }
        }
        // 404 before login; connection refused when a stale lockfile remains.
        Err(e) if e.contains("404") => ConnectionStatus::NotLoggedIn,
        Err(_) => ConnectionStatus::NotRunning,
    }
}

/// Offsets Riot uses for the mirrored copies of skins/champions in the inventory.
pub const SKIN_CLONE_OFFSET: i64 = 60_000_000;
const CHAMPION_CLONE_OFFSET: i64 = 60_000;

fn is_owned_skin(skin: &Value) -> bool {
    skin["isBase"].as_bool() != Some(true) && skin["ownership"]["owned"].as_bool() == Some(true)
}

/// Placeholder price the store catalog gives rewards that were never sold.
const UNPURCHASABLE_RP: i64 = 999_999;

/// Rarity-based RP estimate for skins missing from the live store catalog.
/// Mythic-tier content is priced in other currencies, so it stays unpriced.
fn rarity_rp(rarity: &str) -> Option<i64> {
    match rarity {
        "kEpic" => Some(1350),
        "kLegendary" => Some(1820),
        "kUltimate" => Some(3250),
        "kMythic" | "kExalted" | "kTranscendent" => None,
        _ => Some(975),
    }
}

pub async fn fetch_collection(dd: &DdState) -> Result<Vec<SkinDto>, String> {
    let lcu = connect()?;

    let summoner = lcu.get("/lol-summoner/v1/current-summoner").await?;
    let summoner_id = summoner["summonerId"]
        .as_i64()
        .ok_or("Not logged in yet")?;

    let skins = lcu
        .get(&format!(
            "/lol-champions/v1/inventories/{summoner_id}/skins-minimal"
        ))
        .await?;
    let skins = skins.as_array().ok_or("Unexpected skins-minimal response")?;

    // Live store prices; tolerate failure (store can be unavailable) and
    // fall back to rarity estimates for everything. The catalog also carries
    // availability dates, and for far more skins than it prices, so harvest both.
    let mut prices: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut availability: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    let mut unpurchasable: std::collections::HashSet<i64> = std::collections::HashSet::new();
    if let Ok(catalog) = lcu
        .get("/lol-store/v1/catalog?inventoryType=[%22CHAMPION_SKIN%22]")
        .await
    {
        for item in catalog.as_array().map(Vec::as_slice).unwrap_or_default() {
            let Some(item_id) = item["itemId"].as_i64() else {
                continue;
            };
            let rp = item["prices"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .find(|p| p["currency"].as_str() == Some("RP"))
                .and_then(|p| p["cost"].as_i64());
            // Rewards that were never sold for RP show up with a placeholder cost
            // (0 for Prestige, 999999 for Victorious) or none at all while inactive.
            // Inactive legacy skins keep their last real price, so `active` alone
            // doesn't tell them apart.
            match rp {
                Some(rp) if (1..UNPURCHASABLE_RP).contains(&rp) => {
                    prices.insert(item_id, rp);
                }
                None if item["active"].as_bool() != Some(false) => {}
                _ => {
                    unpurchasable.insert(item_id);
                }
            }
            if let Some(date) = item["releaseDate"].as_str().filter(|d| !d.is_empty()) {
                availability.insert(item_id, date.to_string());
            }
        }
    }

    let champions = dd.champions().await?;
    let meta = dd.skin_meta().await?;

    // The inventory lists most skins twice: once under their real id and once as a
    // clone offset by 60_000_000 on a mirrored champion id (Panda Annie is both
    // 1008/champ 1 and 60001008/champ 60001). Collapsing them is what makes our
    // count match the client's own "skins owned" figure. Collect the real ids up
    // front so the dedupe doesn't depend on the order the client returns.
    let real_ids: std::collections::HashSet<i64> = skins
        .iter()
        .filter(|s| is_owned_skin(s))
        .filter_map(|s| s["id"].as_i64())
        .filter(|id| *id < SKIN_CLONE_OFFSET)
        .collect();

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for skin in skins {
        if !is_owned_skin(skin) {
            continue;
        }
        let Some(raw_id) = skin["id"].as_i64() else {
            continue;
        };
        let skin_id = raw_id % SKIN_CLONE_OFFSET;
        let is_clone_only = raw_id >= SKIN_CLONE_OFFSET;
        if is_clone_only && real_ids.contains(&skin_id) {
            continue;
        }
        if !seen.insert(skin_id) {
            continue;
        }
        let raw_champion_id = skin["championId"].as_i64().unwrap_or(raw_id / 1000);
        let champion_id = raw_champion_id % CHAMPION_CLONE_OFFSET;
        // A few skins exist only in clone form, and CommunityDragon keys those by
        // the clone id, so try both before giving up.
        let (rarity, is_legacy) = meta
            .get(&skin_id)
            .or_else(|| meta.get(&raw_id))
            .map(|m| (m.rarity.clone(), m.is_legacy))
            .unwrap_or_else(|| ("kNoRarity".to_string(), false));

        let is_unpurchasable =
            unpurchasable.contains(&skin_id) || unpurchasable.contains(&raw_id);
        let (rp, value_source) = if is_clone_only || is_unpurchasable {
            // What survives here is the pre-VGU "Classic <champion>" base art. The
            // client counts it as a skin owned, but it was never sold for RP, so it
            // must not pick up a rarity estimate. Same for rewards the store marks as
            // never sold for RP (Victorious, Prestige, event skins).
            (None, "none")
        } else {
            match prices.get(&skin_id).or_else(|| prices.get(&raw_id)) {
                Some(rp) => (Some(*rp), "store"),
                None => match rarity_rp(&rarity) {
                    Some(rp) => (Some(rp), "estimate"),
                    None => (None, "none"),
                },
            }
        };

        out.push(SkinDto {
            skin_id,
            name: skin["name"].as_str().unwrap_or("Unknown").to_string(),
            champion_id,
            champion_name: champions
                .get(&champion_id)
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            rp,
            value_source: value_source.to_string(),
            rarity,
            is_legacy,
            availability_date: availability
                .get(&skin_id)
                .or_else(|| availability.get(&raw_id))
                .cloned(),
        });
    }
    Ok(out)
}
