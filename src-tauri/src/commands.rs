use serde::Serialize;
use tauri::Manager;

use crate::ddragon::DdState;
use crate::lcu::{self, ConnectionStatus, SkinDto};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionDto {
    pub champion_id: i64,
    pub name: String,
}

#[tauri::command]
pub async fn lcu_status() -> ConnectionStatus {
    lcu::status().await
}

#[tauri::command]
pub async fn fetch_collection(dd: tauri::State<'_, DdState>) -> Result<Vec<SkinDto>, String> {
    lcu::fetch_collection(&dd).await
}

#[tauri::command]
pub async fn list_champions(dd: tauri::State<'_, DdState>) -> Result<Vec<ChampionDto>, String> {
    let mut out: Vec<ChampionDto> = dd
        .champions()
        .await?
        .iter()
        .map(|(id, c)| ChampionDto {
            champion_id: *id,
            name: c.name.clone(),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[tauri::command]
pub async fn get_skin_image(
    app: tauri::AppHandle,
    dd: tauri::State<'_, DdState>,
    skin_id: i64,
    kind: String,
) -> Result<String, String> {
    let cache_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("image-cache");
    let path = dd.skin_image(cache_dir, skin_id, &kind).await?;
    Ok(path.to_string_lossy().into_owned())
}
