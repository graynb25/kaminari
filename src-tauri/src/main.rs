// Kaminari Tauri entry. Rust owns IO, DB, net, ext host. Keep thin.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anilist;
mod commands;
mod db;
mod error;
mod ext_host;
mod registry;

use tauri::Manager;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .manage(anilist::AniListClient::new())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("kaminari.db");
            let database = db::Db::new(db_path.to_string_lossy().as_ref())?;
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::boot_snapshot,
            commands::anilist_trending,
            commands::anilist_shelf,
            commands::anilist_search,
            commands::anilist_detail,
            commands::library_list,
            commands::library_add,
            commands::library_remove,
            commands::watched_list,
            commands::watched_set,
            commands::watched_mark_all,
            commands::continue_watching,
            commands::source_validate_id,
            commands::registry_fetch,
            commands::source_list,
            commands::source_install,
            commands::source_remove,
            commands::source_toggle
        ])
        .run(tauri::generate_context!())
        .expect("error starting Kaminari");
}
