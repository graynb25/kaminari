// Commands exposed to Svelte. Payloads in camelCase, typed, no secrets here.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::anilist::{AniListAuthStatus, AniListClient, Media, ShelfKind, Viewer};
use crate::db::{
    ContinueItem, Db, HistoryItem, InstalledSource, JsExtensionRow, LibraryItem,
    SourceContinueItem, SourceHistoryItem, SourceLibraryItem, SourceProgress,
};
use crate::error::AppError;
use crate::ext_host::{self, ExtPackage, ExtStorage};
use crate::player::{self, PlaybackInfo, PlaybackRequest, PlayerState, PlayerStatus};
use crate::proxy::{ProxyStatus, ProxyStore, ProxyStreamInput, ProxyUrl};
use crate::registry::{fetch_index, LegacyExtension};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostLimits {
    pub ext_timeout_secs: u64,
    pub ext_resolve_timeout_secs: u64,
    pub ext_memory_bytes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootSnapshot {
    pub title: String,
    pub anilist_endpoint: String,
    pub limits: HostLimits,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupExport {
    pub path: String,
    pub bytes: u64,
}

const SETTINGS_MAX_BYTES: usize = 64 * 1024;
const ANILIST_KEYRING_SERVICE: &str = "app.kaminari.desktop";
const ANILIST_KEYRING_USER: &str = "anilist";

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(ANILIST_KEYRING_SERVICE, ANILIST_KEYRING_USER)
        .map_err(|err| format!("cannot open the AniList keychain entry: {err}"))
}

fn load_anilist_token() -> Result<Option<String>, String> {
    match keyring_entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(format!(
            "cannot read the AniList token from the keychain: {err}"
        )),
    }
}

fn store_anilist_token(token: &str) -> Result<(), String> {
    keyring_entry()?
        .set_password(token)
        .map_err(|err| format!("cannot save the AniList token in the keychain: {err}"))
}

fn remove_anilist_token() -> Result<(), String> {
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(format!(
            "cannot remove the AniList token from the keychain: {err}"
        )),
    }
}

async fn validate_anilist_token(client: &AniListClient, token: &str) -> Result<Viewer, AppError> {
    client.set_token(token);
    let result = client.viewer().await;
    if result.is_err() {
        client.clear_token();
    }
    result
}

pub async fn anilist_auth_status_for(client: &AniListClient) -> AniListAuthStatus {
    let token = match load_anilist_token() {
        Ok(Some(token)) if !token.trim().is_empty() => token,
        Ok(_) => {
            client.clear_token();
            return AniListAuthStatus {
                connected: false,
                viewer: None,
                error: None,
            };
        }
        Err(err) => {
            client.clear_token();
            return AniListAuthStatus {
                connected: false,
                viewer: None,
                error: Some(err),
            };
        }
    };

    match validate_anilist_token(client, &token).await {
        Ok(viewer) => AniListAuthStatus {
            connected: true,
            viewer: Some(viewer),
            error: None,
        },
        Err(AppError::Auth(message)) => {
            let error = match remove_anilist_token() {
                Ok(()) => message,
                Err(clear_err) => format!("{message} {clear_err}"),
            };
            AniListAuthStatus {
                connected: false,
                viewer: None,
                error: Some(error),
            }
        }
        Err(err) => AniListAuthStatus {
            connected: true,
            viewer: None,
            error: Some(format!(
                "Could not reach AniList. The saved token was kept. {err}"
            )),
        },
    }
}

pub async fn complete_anilist_auth(app: &AppHandle, token: String) -> AniListAuthStatus {
    let client = app.state::<AniListClient>();
    let status = match validate_anilist_token(client.inner(), &token).await {
        Ok(viewer) => match store_anilist_token(&token) {
            Ok(()) => {
                client.set_token(&token);
                AniListAuthStatus {
                    connected: true,
                    viewer: Some(viewer),
                    error: None,
                }
            }
            Err(err) => {
                client.clear_token();
                AniListAuthStatus {
                    connected: false,
                    viewer: None,
                    error: Some(err),
                }
            }
        },
        Err(AppError::Auth(message)) => AniListAuthStatus {
            connected: false,
            viewer: None,
            error: Some(message),
        },
        Err(err) => AniListAuthStatus {
            connected: false,
            viewer: None,
            error: Some(format!(
                "Could not reach AniList. Try connecting again. {err}"
            )),
        },
    };
    let _ = app.emit("anilist-auth-changed", &status);
    status
}

pub async fn handle_anilist_deep_link(app: AppHandle, url: url::Url) {
    let Ok(token) = crate::anilist::parse_auth_callback(&url) else {
        return;
    };
    complete_anilist_auth(&app, token).await;
}

fn ipc_error(error: AppError) -> String {
    error.to_string()
}

#[tauri::command]
pub async fn boot_snapshot(state: State<'_, AniListClient>) -> Result<BootSnapshot, String> {
    Ok(BootSnapshot {
        title: "Kaminari (雷)".to_string(),
        anilist_endpoint: state.endpoint().to_string(),
        limits: HostLimits {
            ext_timeout_secs: ext_host::TIMEOUT_SECS,
            ext_resolve_timeout_secs: ext_host::RESOLVE_TIMEOUT_SECS,
            ext_memory_bytes: ext_host::MEMORY_BYTES,
        },
    })
}

#[tauri::command]
pub async fn anilist_trending(
    state: State<'_, AniListClient>,
    page: Option<u32>,
) -> Result<Vec<Media>, String> {
    state.trending(page.unwrap_or(1)).await.map_err(ipc_error)
}

#[tauri::command]
pub async fn anilist_shelf(
    state: State<'_, AniListClient>,
    kind: String,
    page: Option<u32>,
) -> Result<Vec<Media>, String> {
    state
        .shelf(ShelfKind::from_key(&kind), page.unwrap_or(1))
        .await
        .map_err(ipc_error)
}

#[tauri::command]
pub async fn anilist_search(
    state: State<'_, AniListClient>,
    query: String,
    page: Option<u32>,
) -> Result<Vec<Media>, String> {
    let trimmed = query.trim();
    if trimmed.len() < 2 {
        return Ok(Vec::new());
    }
    state
        .search(trimmed, page.unwrap_or(1))
        .await
        .map_err(ipc_error)
}

#[tauri::command]
pub async fn anilist_detail(state: State<'_, AniListClient>, id: i64) -> Result<Media, String> {
    state.detail(id).await.map_err(ipc_error)
}

#[tauri::command]
pub fn anilist_auth_start(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;

    let url = crate::anilist::auth_url();
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|err| format!("cannot open the AniList sign in page: {err}"))?;
    Ok(url.to_string())
}

#[tauri::command]
pub async fn anilist_auth_status(
    state: State<'_, AniListClient>,
) -> Result<AniListAuthStatus, String> {
    Ok(anilist_auth_status_for(state.inner()).await)
}

#[tauri::command]
pub fn anilist_auth_logout(state: State<'_, AniListClient>) -> Result<(), String> {
    state.clear_token();
    remove_anilist_token()
}

#[tauri::command]
pub fn library_list(state: State<'_, Db>) -> Result<Vec<LibraryItem>, String> {
    state.list_library().map_err(ipc_error)
}

#[tauri::command]
pub fn library_add(
    state: State<'_, Db>,
    media: Media,
    status: Option<String>,
) -> Result<(), String> {
    let status = status.unwrap_or_else(|| "watching".to_string());
    state.add_to_library(&media, &status).map_err(ipc_error)
}

#[tauri::command]
pub fn library_remove(state: State<'_, Db>, id: i64) -> Result<(), String> {
    state.remove_from_library(id).map_err(ipc_error)
}

#[tauri::command]
pub fn library_set_status(state: State<'_, Db>, id: i64, status: String) -> Result<(), String> {
    state.set_library_status(id, &status).map_err(ipc_error)
}

#[tauri::command]
pub fn watched_list(state: State<'_, Db>, anilist_id: i64) -> Result<Vec<i64>, String> {
    state.watched_list(anilist_id).map_err(ipc_error)
}

#[tauri::command]
pub fn watched_set(
    state: State<'_, Db>,
    media: Media,
    episode: i64,
    watched: bool,
) -> Result<Vec<i64>, String> {
    state
        .set_watched(&media, episode, watched)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn watched_mark_all(
    state: State<'_, Db>,
    media: Media,
    through: i64,
    watched: bool,
) -> Result<Vec<i64>, String> {
    state
        .mark_all_watched(&media, through, watched)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn continue_watching(state: State<'_, Db>) -> Result<Vec<ContinueItem>, String> {
    state.continue_watching(20).map_err(ipc_error)
}

#[tauri::command]
pub fn history_list(
    state: State<'_, Db>,
    limit: Option<usize>,
) -> Result<Vec<HistoryItem>, String> {
    state.history_list(limit.unwrap_or(100)).map_err(ipc_error)
}

#[tauri::command]
pub fn source_validate_id(id: String) -> bool {
    ext_host::validate_source_id(&id)
}

#[tauri::command]
pub async fn registry_fetch(url: String) -> Result<Vec<LegacyExtension>, String> {
    fetch_index(&url).await.map_err(ipc_error)
}

#[tauri::command]
pub fn source_list(state: State<'_, Db>) -> Result<Vec<InstalledSource>, String> {
    state.list_sources().map_err(ipc_error)
}

#[tauri::command]
pub fn source_install(state: State<'_, Db>, extension: LegacyExtension) -> Result<(), String> {
    state.install_source(&extension).map_err(ipc_error)
}

#[tauri::command]
pub fn source_remove(state: State<'_, Db>, id: String) -> Result<(), String> {
    state.remove_source(&id).map_err(ipc_error)
}

#[tauri::command]
pub fn source_toggle(state: State<'_, Db>, id: String, enabled: bool) -> Result<(), String> {
    state.toggle_source(&id, enabled).map_err(ipc_error)
}

#[tauri::command]
pub fn js_extension_list(state: State<'_, Db>) -> Result<Vec<JsExtensionRow>, String> {
    state.list_js_extensions().map_err(ipc_error)
}

#[tauri::command]
pub fn js_extension_install_sample(state: State<'_, Db>) -> Result<(), String> {
    let package = ext_host::sample_package().map_err(ipc_error)?;
    state
        .install_js_extension(&package.manifest, &package.code, "builtin")
        .map_err(ipc_error)
}

#[tauri::command]
pub fn js_extension_install_path(state: State<'_, Db>, path: String) -> Result<(), String> {
    let bytes = std::fs::read(&path)
        .map_err(|err| AppError::Extension(format!("cannot read package: {err}")).to_string())?;
    let package = ext_host::package_from_kext(&bytes).map_err(ipc_error)?;
    state
        .install_js_extension(&package.manifest, &package.code, "sideloaded")
        .map_err(ipc_error)
}

#[tauri::command]
pub fn js_extension_remove(state: State<'_, Db>, id: String) -> Result<(), String> {
    state.remove_js_extension(&id).map_err(ipc_error)
}

#[tauri::command]
pub fn js_extension_test(
    state: State<'_, Db>,
    id: String,
    query: String,
) -> Result<serde_json::Value, String> {
    run_js_extension(state.inner(), &id, "search", json!([query, 1]))
}

#[tauri::command]
pub fn js_extension_detail(
    state: State<'_, Db>,
    id: String,
    show_id: String,
) -> Result<Value, String> {
    run_js_extension(state.inner(), &id, "getShow", json!([show_id]))
}

#[tauri::command]
pub fn js_extension_episodes(
    state: State<'_, Db>,
    id: String,
    show_id: String,
) -> Result<Value, String> {
    run_js_extension(state.inner(), &id, "getEpisodes", json!([show_id]))
}

#[tauri::command]
pub fn js_extension_resolve(
    state: State<'_, Db>,
    id: String,
    episode_id: String,
) -> Result<Value, String> {
    run_js_extension(state.inner(), &id, "resolveEpisode", json!([episode_id]))
}

fn run_js_extension(db: &Db, id: &str, op: &str, args: Value) -> Result<Value, String> {
    let stored = db
        .js_extension(id)
        .map_err(ipc_error)?
        .ok_or_else(|| "extension is not installed".to_string())?;
    let storage: Arc<dyn ExtStorage> = Arc::new(db.clone());
    let package = ExtPackage {
        manifest: stored.manifest,
        code: stored.code,
    };
    ext_host::run(
        &package,
        op,
        args,
        storage,
        &ext_host::HostLimits::default(),
    )
    .map_err(ipc_error)
}

#[tauri::command]
pub async fn proxy_register(
    state: State<'_, ProxyStore>,
    db: State<'_, Db>,
    extension_id: String,
    stream: ProxyStreamInput,
) -> Result<ProxyUrl, String> {
    let stored = db
        .js_extension(&extension_id)
        .map_err(ipc_error)?
        .ok_or_else(|| "extension is not installed".to_string())?;
    let allowed = stored.manifest.allowed_rules().map_err(ipc_error)?;
    state.register(&extension_id, &stream, allowed).await
}

#[tauri::command]
pub fn proxy_revoke(state: State<'_, ProxyStore>, opaque_id: String) -> Result<(), String> {
    state.revoke(&opaque_id)
}

#[tauri::command]
pub fn proxy_status(state: State<'_, ProxyStore>) -> Result<ProxyStatus, String> {
    Ok(state.status())
}

#[tauri::command]
pub fn player_play(
    state: State<'_, PlayerState>,
    request: PlaybackRequest,
) -> Result<PlaybackInfo, String> {
    state.play(&request).map_err(ipc_error)
}

#[tauri::command]
pub fn player_stop(state: State<'_, PlayerState>) {
    state.stop();
}

#[tauri::command]
pub fn player_status() -> PlayerStatus {
    player::status()
}

#[tauri::command]
pub fn source_library_list(state: State<'_, Db>) -> Result<Vec<SourceLibraryItem>, String> {
    state.source_library_list().map_err(ipc_error)
}

#[tauri::command]
pub fn source_library_add(
    state: State<'_, Db>,
    extension_id: String,
    show_id: String,
    title: String,
    cover: Option<String>,
    year: Option<i32>,
    status: Option<String>,
) -> Result<(), String> {
    state
        .source_library_add(
            &extension_id,
            &show_id,
            &title,
            cover.as_deref(),
            year,
            status.as_deref(),
        )
        .map_err(ipc_error)
}

#[tauri::command]
pub fn source_library_remove(
    state: State<'_, Db>,
    extension_id: String,
    show_id: String,
) -> Result<(), String> {
    state
        .source_library_remove(&extension_id, &show_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn source_progress_get(
    state: State<'_, Db>,
    extension_id: String,
    show_id: String,
) -> Result<Vec<SourceProgress>, String> {
    state
        .source_progress_get(&extension_id, &show_id)
        .map_err(ipc_error)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn source_progress_set(
    state: State<'_, Db>,
    extension_id: String,
    show_id: String,
    episode_id: String,
    episode_number: i64,
    episode_title: Option<String>,
    position_sec: f64,
    duration_sec: f64,
    watched: bool,
) -> Result<(), String> {
    state
        .source_progress_set(
            &extension_id,
            &show_id,
            &episode_id,
            episode_number,
            episode_title.as_deref(),
            position_sec,
            duration_sec,
            watched,
        )
        .map_err(ipc_error)
}

#[tauri::command]
pub fn source_continue(
    state: State<'_, Db>,
    limit: Option<usize>,
) -> Result<Vec<SourceContinueItem>, String> {
    state
        .source_continue(limit.unwrap_or(20))
        .map_err(ipc_error)
}

#[tauri::command]
pub fn source_history(
    state: State<'_, Db>,
    limit: Option<usize>,
) -> Result<Vec<SourceHistoryItem>, String> {
    state
        .source_history(limit.unwrap_or(100))
        .map_err(ipc_error)
}

#[tauri::command]
pub fn backup_export(state: State<'_, Db>, path: String) -> Result<BackupExport, String> {
    let bytes = state.backup_to(&path).map_err(ipc_error)?;
    Ok(BackupExport { path, bytes })
}

#[tauri::command]
pub fn backup_restore(state: State<'_, Db>, path: String) -> Result<(), String> {
    state.restore_from(&path).map_err(ipc_error)
}

#[tauri::command]
pub fn settings_export(path: String, json: String) -> Result<(), String> {
    if json.len() > SETTINGS_MAX_BYTES {
        return Err("settings file is too large (64 KiB limit)".to_string());
    }
    let target = settings_path(&path)?;
    write_atomic(&target, json.as_bytes())
}

#[tauri::command]
pub fn settings_read(path: String) -> Result<String, String> {
    let target = settings_path(&path)?;
    let metadata =
        std::fs::metadata(&target).map_err(|err| format!("cannot read settings file: {err}"))?;
    if metadata.len() > SETTINGS_MAX_BYTES as u64 {
        return Err("settings file is too large (64 KiB limit)".to_string());
    }
    let bytes =
        std::fs::read(&target).map_err(|err| format!("cannot read settings file: {err}"))?;
    if bytes.len() > SETTINGS_MAX_BYTES {
        return Err("settings file is too large (64 KiB limit)".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "settings file is not UTF-8".to_string())
}

#[tauri::command]
pub fn app_restart(app: AppHandle) {
    app.restart();
}

fn settings_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("settings path is empty".to_string());
    }
    let target = PathBuf::from(trimmed);
    if !target.is_absolute() {
        return Err("settings path must be absolute".to_string());
    }
    Ok(target)
}

fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| "settings path has no file name".to_string())?;
    let temp = target.with_file_name(format!(".{name}.kaminari.tmp"));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temp, target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|err| format!("cannot write settings file: {err}"))
}

fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    match std::fs::rename(temp, target) {
        Ok(()) => Ok(()),
        Err(err)
            if err.kind() == std::io::ErrorKind::AlreadyExists
                || err.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            std::fs::remove_file(target)?;
            std::fs::rename(temp, target)
        }
        Err(err) => Err(err),
    }
}
