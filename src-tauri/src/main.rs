// Kaminari Tauri entry. Rust owns IO, DB, net, ext host. Keep thin.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anilist;
mod commands;
mod db;
mod download;
mod error;
mod ext_host;
mod mpv_embed;
mod player;
mod proxy;
mod registry;
mod tray;

use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

fn handle_deep_links(handle: &tauri::AppHandle, urls: Vec<url::Url>) {
    for url in urls {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            commands::handle_anilist_deep_link(handle, url).await;
        });
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_libmpv::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(anilist::AniListClient::new())
        .manage(player::PlayerState::new())
        .manage(mpv_embed::MpvEmbedState::default())
        .manage(download::DownloadState::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("kaminari.db");
            let database = db::Db::new(db_path.to_string_lossy().as_ref())?;
            app.manage(database);

            let sync_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    commands::anilist_sync_tick(&sync_handle).await;
                }
            });

            let (proxy, server) = proxy::ProxyStore::bind();
            if let Some((listener, router)) = server {
                let guard = proxy.clone();
                tauri::async_runtime::spawn(async move {
                    let listener = match tokio::net::TcpListener::from_std(listener) {
                        Ok(listener) => listener,
                        Err(err) => {
                            tracing::error!(error = %err, "proxy: listener could not start");
                            guard.disable();
                            return;
                        }
                    };
                    if let Err(err) = axum::serve(listener, router).await {
                        tracing::error!(error = %err, "proxy: server stopped");
                        guard.disable();
                    }
                });
            }
            let sweeper = proxy.clone();
            tauri::async_runtime::spawn(async move {
                sweeper.sweep_loop().await;
            });
            app.manage(proxy);

            if let Err(err) = tray::setup(app.handle()) {
                tracing::warn!(error = %err, "tray: setup failed");
            }

            let handle = app.handle().clone();
            app.deep_link().on_open_url({
                let handle = handle.clone();
                move |event| handle_deep_links(&handle, event.urls())
            });

            #[cfg(any(target_os = "linux", all(target_os = "windows", debug_assertions)))]
            if let Err(err) = app.deep_link().register_all() {
                tracing::warn!(error = %err, "deep link: scheme registration failed");
            }

            if let Ok(Some(urls)) = app.deep_link().get_current() {
                handle_deep_links(&handle, urls);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::boot_snapshot,
            commands::anilist_trending,
            commands::anilist_shelf,
            commands::anilist_search,
            commands::anilist_detail,
            commands::anilist_auth_start,
            commands::anilist_auth_status,
            commands::anilist_auth_logout,
            commands::anilist_sync_pull,
            commands::anilist_sync_push,
            commands::anilist_sync_pending,
            commands::library_list,
            commands::library_add,
            commands::library_remove,
            commands::library_set_status,
            commands::watched_list,
            commands::watched_set,
            commands::watched_mark_all,
            commands::continue_watching,
            commands::history_list,
            commands::source_validate_id,
            commands::registry_fetch,
            commands::source_list,
            commands::source_install,
            commands::source_remove,
            commands::source_toggle,
            commands::js_extension_list,
            commands::js_extension_install_sample,
            commands::js_extension_install_path,
            commands::js_extension_remove,
            commands::js_extension_test,
            commands::js_extension_detail,
            commands::js_extension_episodes,
            commands::js_extension_resolve,
            commands::proxy_register,
            commands::proxy_revoke,
            commands::proxy_status,
            commands::player_play,
            commands::player_stop,
            commands::player_status,
            mpv_embed::mpv_embed_status,
            mpv_embed::mpv_embed_init,
            mpv_embed::mpv_embed_load,
            mpv_embed::mpv_embed_set,
            mpv_embed::mpv_embed_seek,
            mpv_embed::mpv_embed_cycle_sub,
            mpv_embed::mpv_embed_get,
            mpv_embed::mpv_embed_destroy,
            commands::source_library_list,
            commands::source_library_add,
            commands::source_library_remove,
            commands::source_progress_get,
            commands::source_progress_set,
            commands::source_continue,
            commands::source_history,
            commands::backup_export,
            commands::backup_restore,
            commands::settings_export,
            commands::settings_read,
            commands::app_restart,
            download::download_start,
            download::download_cancel,
            download::downloads_list
        ])
        .run(tauri::generate_context!())
        .expect("error starting Kaminari");
}
