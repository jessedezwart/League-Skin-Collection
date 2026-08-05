mod commands;
mod ddragon;
mod lcu;

use ddragon::DdState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(DdState::default())
        .invoke_handler(tauri::generate_handler![
            commands::lcu_status,
            commands::fetch_collection,
            commands::list_champions,
            commands::get_skin_image
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
