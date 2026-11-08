// Kaminari Tauri entry. Rust owns IO, DB, net, ext host. Keep thin.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anilist;
mod commands;
mod db;
mod error;
mod ext_host;

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
            match db::open(db_path.to_string_lossy().as_ref()) {
                Ok(mut conn) => {
                    if let Err(err) = db::apply_migrations(&mut conn) {
                        tracing::error!(%err, "database migration failed");
                    }
                }
                Err(err) => tracing::error!(%err, "database open failed"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::boot_snapshot,
            commands::source_validate_id
        ])
        .run(tauri::generate_context!())
        .expect("error starting Kaminari");
}
