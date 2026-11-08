// Kaminari Tauri entry. Rust owns IO, DB, net, ext host. Keep thin.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod anilist;
mod ext_host;
mod error;

use commands::boot_snapshot;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![boot_snapshot])
        .run(tauri::generate_context!())
        .expect("error starting Kaminari");
}
