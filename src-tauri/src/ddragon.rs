use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::OnceCell;

use crate::lcu::SKIN_CLONE_OFFSET;

/// Root that mirrors the client's own `/lol-game-data/assets/` tree.
const CDRAGON_ASSETS: &str =
    "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default";

#[derive(Clone)]
pub struct ChampionRef {
    /// URL-safe champion id used in ddragon image paths, e.g. "MonkeyKing"
    pub id: String,
    /// Display name, e.g. "Wukong"
    pub name: String,
}

#[derive(Clone)]
pub struct SkinMeta {
    pub rarity: String,
    pub is_legacy: bool,
    /// Game-data asset path of the vertical load-screen art, if any.
    pub load_screen_path: Option<String>,
    /// Game-data asset path of the centered splash, if any.
    pub splash_path: Option<String>,
}

pub struct DdState {
    pub http: reqwest::Client,
    version: OnceCell<String>,
    champions: OnceCell<HashMap<i64, ChampionRef>>,
    /// skinId -> metadata from CommunityDragon
    skin_meta: OnceCell<HashMap<i64, SkinMeta>>,
}

impl Default for DdState {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
            version: OnceCell::new(),
            champions: OnceCell::new(),
            skin_meta: OnceCell::new(),
        }
    }
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl DdState {
    async fn fetch_json(&self, url: &str) -> Result<Value, String> {
        self.http
            .get(url)
            .send()
            .await
            .map_err(err)?
            .error_for_status()
            .map_err(err)?
            .json()
            .await
            .map_err(err)
    }

    pub async fn version(&self) -> Result<&String, String> {
        self.version
            .get_or_try_init(|| async {
                let v = self
                    .fetch_json("https://ddragon.leagueoflegends.com/api/versions.json")
                    .await?;
                v.get(0)
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| "ddragon versions.json was empty".to_string())
            })
            .await
    }

    pub async fn champions(&self) -> Result<&HashMap<i64, ChampionRef>, String> {
        self.champions
            .get_or_try_init(|| async {
                let version = self.version().await?;
                let url = format!(
                    "https://ddragon.leagueoflegends.com/cdn/{version}/data/en_US/champion.json"
                );
                let v = self.fetch_json(&url).await?;
                let data = v
                    .get("data")
                    .and_then(Value::as_object)
                    .ok_or("champion.json missing data")?;
                let mut map = HashMap::new();
                for champ in data.values() {
                    let key: i64 = champ["key"]
                        .as_str()
                        .and_then(|k| k.parse().ok())
                        .ok_or("champion missing key")?;
                    // ddragon lists cosmetic clones of real champions ("Jade_Ahri" under key
                    // 60103 next to "Ahri" under 103). They share the display name, so keeping
                    // them would duplicate every affected champion. Real keys are all < 1000.
                    if key >= 10000 {
                        continue;
                    }
                    map.insert(
                        key,
                        ChampionRef {
                            id: champ["id"].as_str().unwrap_or_default().to_string(),
                            name: champ["name"].as_str().unwrap_or_default().to_string(),
                        },
                    );
                }
                Ok(map)
            })
            .await
    }

    pub async fn skin_meta(&self) -> Result<&HashMap<i64, SkinMeta>, String> {
        self.skin_meta
            .get_or_try_init(|| async {
                let v = self
                    .fetch_json("https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/skins.json")
                    .await?;
                let obj = v.as_object().ok_or("cdragon skins.json not an object")?;
                let mut map = HashMap::new();
                for (id, skin) in obj {
                    if let Ok(id) = id.parse::<i64>() {
                        let asset = |key: &str| {
                            skin[key]
                                .as_str()
                                .filter(|p| !p.is_empty())
                                .map(str::to_string)
                        };
                        map.insert(
                            id,
                            SkinMeta {
                                rarity: skin["rarity"].as_str().unwrap_or("kNoRarity").to_string(),
                                is_legacy: skin["isLegacy"].as_bool().unwrap_or(false),
                                load_screen_path: asset("loadScreenPath"),
                                splash_path: asset("splashPath"),
                            },
                        );
                    }
                }
                Ok(map)
            })
            .await
    }

    /// Returns the local path of a cached skin image, downloading it on first use.
    /// `kind` is "loading" (card) or "splash" (full art).
    pub async fn skin_image(
        &self,
        cache_dir: PathBuf,
        skin_id: i64,
        kind: &str,
    ) -> Result<PathBuf, String> {
        if kind != "loading" && kind != "splash" {
            return Err(format!("unknown image kind: {kind}"));
        }
        let path = cache_dir.join(format!("{skin_id}_{kind}.jpg"));
        if path.exists() {
            return Ok(path);
        }

        let mut urls = Vec::new();
        let champ_id = skin_id / 1000;
        let num = skin_id % 1000;
        if let Some(champ) = self.champions().await?.get(&champ_id) {
            urls.push(format!(
                "https://ddragon.leagueoflegends.com/cdn/img/champion/{kind}/{}_{num}.jpg",
                champ.id
            ));
        }
        // ddragon has never heard of the "Classic <champion>" skins — they only exist
        // as game-data assets under a Jade_* character. CommunityDragon mirrors that
        // tree, and keys their metadata by the clone id the inventory id came from.
        let meta = self.skin_meta().await?;
        if let Some(m) = meta
            .get(&skin_id)
            .or_else(|| meta.get(&(skin_id + SKIN_CLONE_OFFSET)))
        {
            let asset = if kind == "loading" {
                &m.load_screen_path
            } else {
                &m.splash_path
            };
            urls.extend(asset.as_deref().and_then(cdragon_url));
        }

        let mut last = format!("no image source for skin {skin_id}");
        for url in urls {
            match self.download(&url).await {
                Ok(bytes) => {
                    std::fs::create_dir_all(&cache_dir).map_err(err)?;
                    std::fs::write(&path, &bytes).map_err(err)?;
                    return Ok(path);
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    async fn download(&self, url: &str) -> Result<Vec<u8>, String> {
        let bytes = self
            .http
            .get(url)
            .send()
            .await
            .map_err(err)?
            .error_for_status()
            .map_err(err)?
            .bytes()
            .await
            .map_err(err)?;
        Ok(bytes.to_vec())
    }
}

/// Rewrites a client asset path (`/lol-game-data/assets/ASSETS/...`) into the
/// CommunityDragon URL that serves the same file. The mirror is all-lowercase.
fn cdragon_url(asset_path: &str) -> Option<String> {
    let rest = asset_path.strip_prefix("/lol-game-data/assets/")?;
    Some(format!("{CDRAGON_ASSETS}/{}", rest.to_lowercase()))
}
