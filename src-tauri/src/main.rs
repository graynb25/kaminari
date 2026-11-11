// Kaminari Tauri entry. Rust owns IO, DB, net, ext host. Keep thin.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anilist;
mod commands;
mod db;
mod download;
mod error;
mod ext_host;
mod fsutil;
mod mangayomi;
mod mpv_embed;
mod path_guard;
mod player;
mod portable;
mod proxy;
#[cfg(debug_assertions)]
mod registry;
mod safe_http;
mod startup;
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

    let result = tauri::Builder::default()
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(anilist::AniListClient::new())
        .manage(anilist::AniListAuthState::new())
        .manage(player::PlayerState::new())
        .manage(mpv_embed::MpvEmbedState::default())
        .manage(download::DownloadState::default())
        .manage(path_guard::PathAuthority::new())
        .manage(commands::ExtensionInstallState::new())
        .manage(commands::AniListSyncGuard::new())
        .setup(|app| {
            let data_dir = match app.path().app_data_dir() {
                Ok(dir) => dir,
                Err(err) => {
                    startup::report_fatal(&format!(
                        "Kaminari could not resolve its data folder: {err}"
                    ));
                    std::process::exit(1);
                }
            };
            let Some(database) = startup::open_database(app.handle(), &data_dir) else {
                // The recovery dialog's cancel/close button quits without
                // touching user data.
                std::process::exit(0);
            };
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
            if !portable::is_portable() {
                if let Err(err) = app.deep_link().register_all() {
                    tracing::warn!(error = %err, "deep link: scheme registration failed");
                }
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
            commands::anilist_oauth_status,
            commands::anilist_register_callback,
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
            commands::history_clear,
            commands::history_remove,
            // Legacy Aniyomi registry/source commands stay available to
            // development builds and tests but are not reachable from the
            // release webview. The metadata code remains for future runtime
            // work.
            #[cfg(debug_assertions)]
            commands::source_validate_id,
            #[cfg(debug_assertions)]
            commands::registry_fetch,
            #[cfg(debug_assertions)]
            commands::source_list,
            #[cfg(debug_assertions)]
            commands::source_install,
            #[cfg(debug_assertions)]
            commands::source_remove,
            #[cfg(debug_assertions)]
            commands::source_toggle,
            commands::js_extension_list,
            commands::js_extension_install_sample,
            commands::js_extension_install_pick,
            commands::js_extension_install_package,
            commands::js_extension_install_confirm,
            commands::mangayomi_preview,
            commands::mangayomi_install,
            commands::js_extension_remove,
            commands::js_extension_test,
            commands::js_extension_preferences,
            commands::js_extension_set_preference,
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
            commands::source_progress_mark_all,
            commands::source_continue,
            commands::source_history,
            commands::source_history_remove,
            commands::backup_export_pick,
            commands::backup_export,
            commands::backup_restore_pick,
            commands::backup_restore,
            commands::settings_export_pick,
            commands::settings_export,
            commands::settings_import_pick,
            commands::settings_read,
            commands::app_restart,
            download::download_start,
            download::download_cancel,
            download::downloads_list
        ])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        startup::report_fatal(&format!("Kaminari stopped before its window opened: {err}"));
        std::process::exit(1);
    }
}
