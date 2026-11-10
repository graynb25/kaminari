// Commands exposed to Svelte. Payloads in camelCase, typed, no secrets here.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Semaphore;

use crate::anilist::{
    decide_write, AniListAuthStatus, AniListClient, Media, ShelfKind, Viewer, WriteDecision,
};
use crate::db::{
    ContinueItem, Db, HistoryItem, InstalledSource, JsExtensionRow, LibraryItem,
    SourceContinueItem, SourceHistoryItem, SourceLibraryItem, SourceProgress, SourceProgressMark,
};
use crate::error::AppError;
use crate::ext_host::{self, ExtPackage, ExtStorage};
use crate::mangayomi;
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AniListSyncResult {
    pub titles: usize,
    pub viewer_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AniListPushResult {
    pub pushed: usize,
    pub conflicts: usize,
    pub pending: usize,
}

const SETTINGS_MAX_BYTES: usize = 64 * 1024;
const ANILIST_KEYRING_SERVICE: &str = "app.kaminari.desktop";
const ANILIST_KEYRING_USER: &str = "anilist";
// One slow source must not occupy the whole blocking pool. Calls beyond this
// limit wait for a permit before they are handed to spawn_blocking.
const EXT_MAX_PARALLEL_PER_SOURCE: usize = 2;

struct ExtensionRunLimiter {
    per_source: Mutex<HashMap<String, Arc<Semaphore>>>,
}

impl ExtensionRunLimiter {
    fn new() -> Self {
        Self {
            per_source: Mutex::new(HashMap::new()),
        }
    }

    fn semaphore_for(&self, id: &str) -> Arc<Semaphore> {
        let mut map = self
            .per_source
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.entry(id.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(EXT_MAX_PARALLEL_PER_SOURCE)))
            .clone()
    }
}

fn extension_run_limiter() -> &'static ExtensionRunLimiter {
    static LIMITER: OnceLock<ExtensionRunLimiter> = OnceLock::new();
    LIMITER.get_or_init(ExtensionRunLimiter::new)
}

async fn run_extension_blocking<T, F>(id: &str, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let semaphore = extension_run_limiter().semaphore_for(id);
    let permit = semaphore
        .acquire_owned()
        .await
        .map_err(|_| "extension runner is unavailable".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        task()
    })
    .await
    .map_err(|err| format!("extension worker failed: {err}"))?
}

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
        Err(AppError::Auth(message)) => failed_callback_status(client.inner(), message).await,
        Err(err) => {
            let message = format!("Could not reach AniList. Try connecting again. {err}");
            failed_callback_status(client.inner(), message).await
        }
    };
    let _ = app.emit("anilist-auth-changed", &status);
    status
}

async fn failed_callback_status(client: &AniListClient, message: String) -> AniListAuthStatus {
    let current = anilist_auth_status_for(client).await;
    if current.connected {
        AniListAuthStatus {
            connected: true,
            viewer: current.viewer,
            error: Some(message),
        }
    } else {
        AniListAuthStatus {
            connected: false,
            viewer: None,
            error: Some(message),
        }
    }
}

pub async fn handle_anilist_deep_link(app: AppHandle, url: url::Url) {
    if !crate::anilist::is_auth_callback(&url) {
        return;
    }
    let Ok(callback) = crate::anilist::parse_auth_callback(&url) else {
        app.state::<crate::anilist::AniListAuthState>().clear();
        reject_anilist_callback(&app, "The AniList sign in response was invalid. Try again.").await;
        return;
    };
    let checked = app
        .state::<crate::anilist::AniListAuthState>()
        .consume(&callback.state);
    if let Err(err) = checked {
        let message = match err {
            crate::anilist::AuthStateError::Expired => "The AniList sign in expired. Try again.",
            crate::anilist::AuthStateError::Missing | crate::anilist::AuthStateError::Mismatch => {
                "The AniList sign in did not match this app. Try again."
            }
        };
        reject_anilist_callback(&app, message).await;
        return;
    }
    complete_anilist_auth(&app, callback.token).await;
}

async fn reject_anilist_callback(app: &AppHandle, message: &str) {
    let client = app.state::<AniListClient>();
    let status = failed_callback_status(client.inner(), message.to_string()).await;
    let _ = app.emit("anilist-auth-changed", &status);
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

    let state = app
        .state::<crate::anilist::AniListAuthState>()
        .begin()
        .map_err(|err| format!("cannot start the AniList sign in: {err}"))?;
    let url = crate::anilist::authorize_url(&state);
    if let Err(err) = app.opener().open_url(url.as_str(), None::<&str>) {
        app.state::<crate::anilist::AniListAuthState>().clear();
        return Err(format!("cannot open the AniList sign in page: {err}"));
    }
    Ok(url)
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
pub async fn anilist_sync_pull(
    state: State<'_, AniListClient>,
    db: State<'_, Db>,
) -> Result<AniListSyncResult, String> {
    let token = load_anilist_token()?
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| "Connect AniList first.".to_string())?;
    state.set_token(&token);
    let viewer = state.viewer().await.map_err(ipc_error)?;
    let entries = state.media_list(viewer.id).await.map_err(ipc_error)?;
    let titles = db.sync_anilist_entries(&entries).map_err(ipc_error)?;
    Ok(AniListSyncResult {
        titles,
        viewer_name: viewer.name,
    })
}

async fn push_pending_anilist(
    client: &AniListClient,
    db: &Db,
) -> Result<AniListPushResult, AppError> {
    let rows = db.pending_anilist_sync()?;
    let mut pushed = 0;
    let mut conflicts = 0;
    for row in rows {
        let server = match client.media_list_entry(row.anilist_id).await {
            Ok(entry) => entry,
            Err(err @ (AppError::Net(_) | AppError::RateLimited(_))) => {
                db.fail_anilist_sync(row.anilist_id, &err.to_string())?;
                continue;
            }
            Err(err) => return Err(err),
        };
        match decide_write(&row.status, row.progress, row.updated_at, server.as_ref()) {
            WriteDecision::SkipServerNewer => {
                if let Some(server) = server.as_ref() {
                    tracing::warn!(
                        media_id = row.anilist_id,
                        local_updated_at = row.updated_at,
                        server_updated_at = server.updated_at,
                        "anilist: server entry is newer, keeping AniList and dropping the local write"
                    );
                }
                db.remove_anilist_sync(row.anilist_id)?;
                conflicts += 1;
            }
            WriteDecision::SkipMatching => {
                db.remove_anilist_sync(row.anilist_id)?;
            }
            WriteDecision::Push => {
                match client
                    .save_media_list_entry(row.anilist_id, &row.status, row.progress)
                    .await
                {
                    Ok(_) => {
                        db.remove_anilist_sync(row.anilist_id)?;
                        pushed += 1;
                    }
                    Err(err @ (AppError::Net(_) | AppError::RateLimited(_))) => {
                        db.fail_anilist_sync(row.anilist_id, &err.to_string())?;
                    }
                    Err(err) => return Err(err),
                }
            }
        }
    }
    let pending = db.pending_anilist_sync_count()?;
    Ok(AniListPushResult {
        pushed,
        conflicts,
        pending,
    })
}

#[tauri::command]
pub async fn anilist_sync_push(
    app: AppHandle,
    state: State<'_, AniListClient>,
    db: State<'_, Db>,
) -> Result<AniListPushResult, String> {
    let token = load_anilist_token()?
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| "Connect AniList first.".to_string())?;
    state.set_token(&token);
    let result = push_pending_anilist(state.inner(), db.inner())
        .await
        .map_err(ipc_error)?;
    if result.conflicts > 0 {
        let _ = app.emit("anilist-sync-conflict", &result);
    }
    Ok(result)
}

#[tauri::command]
pub fn anilist_sync_pending(db: State<'_, Db>) -> Result<usize, String> {
    db.pending_anilist_sync_count().map_err(ipc_error)
}

pub async fn anilist_sync_tick(app: &AppHandle) {
    let db = app.state::<Db>();
    match db.pending_anilist_sync_count() {
        Ok(0) => return,
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(error = %err, "anilist: sync queue could not be read");
            return;
        }
    }
    let token = match load_anilist_token() {
        Ok(Some(token)) if !token.trim().is_empty() => token,
        Ok(_) => {
            app.state::<AniListClient>().clear_token();
            return;
        }
        Err(err) => {
            tracing::warn!(error = %err, "anilist: token could not be read");
            return;
        }
    };
    let client = app.state::<AniListClient>();
    client.set_token(&token);
    match push_pending_anilist(client.inner(), db.inner()).await {
        Ok(result) if result.conflicts > 0 => {
            let _ = app.emit("anilist-sync-conflict", &result);
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(error = %err, "anilist: background sync stopped, queue kept");
        }
    }
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
pub fn history_clear(state: State<'_, Db>) -> Result<(), String> {
    state.history_clear().map_err(ipc_error)
}

#[tauri::command]
pub fn history_remove(state: State<'_, Db>, anilist_id: i64, episode: i64) -> Result<(), String> {
    state.history_remove(anilist_id, episode).map_err(ipc_error)
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
pub async fn js_extension_install_path(state: State<'_, Db>, path: String) -> Result<(), String> {
    let db = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(&path).map_err(|err| {
            AppError::Extension(format!("cannot read package: {err}")).to_string()
        })?;
        let package = ext_host::package_from_kext(&bytes).map_err(ipc_error)?;
        db.install_js_extension(&package.manifest, &package.code, "sideloaded")
            .map_err(ipc_error)
    })
    .await
    .map_err(|err| format!("extension install failed: {err}"))?
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MangayomiPreview {
    pub url: String,
    pub sha256: String,
    pub bytes: usize,
    pub sources: Vec<mangayomi::PreviewSource>,
}

#[tauri::command]
pub async fn mangayomi_preview(url: String) -> Result<MangayomiPreview, String> {
    let (code, sha256, bytes) = fetch_mangayomi_source(&url).await?;
    let preview = mangayomi::preview(&code).map_err(ipc_error)?;
    Ok(MangayomiPreview {
        url,
        sha256,
        bytes,
        sources: preview.sources,
    })
}

#[tauri::command]
pub async fn mangayomi_install(
    state: State<'_, Db>,
    url: String,
    source_index: usize,
    sha256: String,
) -> Result<String, String> {
    let (code, actual, _) = fetch_mangayomi_source(&url).await?;
    if actual != sha256 {
        return Err(
            "the source changed since inspection; inspect it again before installing".to_string(),
        );
    }
    let source = mangayomi::source_at(&code, source_index).map_err(ipc_error)?;
    let manifest = mangayomi::build_manifest(&source, source_index).map_err(ipc_error)?;
    let wrapped = mangayomi::wrap_code(&code, source_index);
    state
        .install_js_extension(&manifest, &wrapped, "sideloaded")
        .map_err(ipc_error)?;
    Ok(manifest.id)
}

async fn fetch_mangayomi_source(url: &str) -> Result<(String, String, usize), String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "source URL is not valid".to_string())?;
    if parsed.scheme() != "https" {
        return Err("only https source URLs are allowed".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("source URL must not contain credentials".to_string());
    }
    if ext_host::is_private_host(parsed.host_str().unwrap_or("")) {
        return Err("source URL host is local or private".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|err| format!("network client failed: {err}"))?;
    let response = client
        .get(parsed)
        .header(reqwest::header::ACCEPT, "text/javascript, text/plain, */*")
        .send()
        .await
        .map_err(|err| format!("source download failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "source download failed with HTTP {}",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > mangayomi::MAX_SOURCE_BYTES as u64)
    {
        return Err("source is larger than 2 MB".to_string());
    }
    let mut body = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| format!("source download failed: {err}"))?
    {
        if body.len() + chunk.len() > mangayomi::MAX_SOURCE_BYTES {
            return Err("source is larger than 2 MB".to_string());
        }
        body.extend_from_slice(&chunk);
    }
    let bytes = body.len();
    let code =
        String::from_utf8(body).map_err(|_| "source is not valid UTF-8 JavaScript".to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    let sha256 = hex::encode(hasher.finalize());
    Ok((code, sha256, bytes))
}

#[tauri::command]
pub async fn js_extension_preferences(state: State<'_, Db>, id: String) -> Result<Value, String> {
    let db = state.inner().clone();
    let worker_id = id.clone();
    run_extension_blocking(&id, move || {
        js_extension_preferences_blocking(&db, &worker_id)
    })
    .await
}

fn js_extension_preferences_blocking(db: &Db, id: &str) -> Result<Value, String> {
    let stored = db
        .js_extension(id)
        .map_err(ipc_error)?
        .ok_or_else(|| "extension is not installed".to_string())?;
    if !stored.manifest.id.starts_with("mangayomi.") {
        return Ok(json!({ "items": [], "values": {} }));
    }
    let items = run_js_extension_blocking(db, id, "getPreferences", json!([]))?;
    let mut values = serde_json::Map::new();
    if let Some(rows) = items.as_array() {
        for row in rows {
            let Some(key) = row.get("key").and_then(Value::as_str) else {
                continue;
            };
            if let Some(value) = ExtStorage::get(db, id, &format!("pref.{key}")) {
                values.insert(key.to_string(), Value::String(value));
            }
        }
    }
    Ok(json!({ "items": items, "values": values }))
}

#[tauri::command]
pub fn js_extension_set_preference(
    state: State<'_, Db>,
    id: String,
    key: String,
    value: String,
) -> Result<(), String> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err("preference key is not valid".to_string());
    }
    if value.len() > 4096 {
        return Err("preference value is too large".to_string());
    }
    let stored = state
        .js_extension(&id)
        .map_err(ipc_error)?
        .ok_or_else(|| "extension is not installed".to_string())?;
    if !stored.manifest.id.starts_with("mangayomi.") {
        return Err("source preferences are only available for Mangayomi sources".to_string());
    }
    ExtStorage::set(state.inner(), &id, &format!("pref.{key}"), &value).map_err(ipc_error)
}

#[tauri::command]
pub fn js_extension_remove(state: State<'_, Db>, id: String) -> Result<(), String> {
    state.remove_js_extension(&id).map_err(ipc_error)
}

#[tauri::command]
pub async fn js_extension_test(
    state: State<'_, Db>,
    id: String,
    query: String,
) -> Result<Value, String> {
    let db = state.inner().clone();
    run_js_extension(db, id, "search", json!([query, 1])).await
}

#[tauri::command]
pub async fn js_extension_detail(
    state: State<'_, Db>,
    id: String,
    show_id: String,
) -> Result<Value, String> {
    let db = state.inner().clone();
    run_js_extension(db, id, "getShow", json!([show_id])).await
}

#[tauri::command]
pub async fn js_extension_episodes(
    state: State<'_, Db>,
    id: String,
    show_id: String,
) -> Result<Value, String> {
    let db = state.inner().clone();
    run_js_extension(db, id, "getEpisodes", json!([show_id])).await
}

#[tauri::command]
pub async fn js_extension_resolve(
    state: State<'_, Db>,
    id: String,
    episode_id: String,
) -> Result<Value, String> {
    let db = state.inner().clone();
    run_js_extension(db, id, "resolveEpisode", json!([episode_id])).await
}

async fn run_js_extension(
    db: Db,
    id: String,
    op: &'static str,
    args: Value,
) -> Result<Value, String> {
    let worker_id = id.clone();
    run_extension_blocking(&id, move || {
        run_js_extension_blocking(&db, &worker_id, op, args)
    })
    .await
}

fn run_js_extension_blocking(db: &Db, id: &str, op: &str, args: Value) -> Result<Value, String> {
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
#[allow(clippy::too_many_arguments)]
pub fn source_library_add(
    state: State<'_, Db>,
    extension_id: String,
    show_id: String,
    title: String,
    cover: Option<String>,
    year: Option<i32>,
    status: Option<String>,
    anilist_id: Option<i64>,
) -> Result<(), String> {
    state
        .source_library_add(
            &extension_id,
            &show_id,
            &title,
            cover.as_deref(),
            year,
            status.as_deref(),
            anilist_id,
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
pub fn source_progress_mark_all(
    state: State<'_, Db>,
    extension_id: String,
    show_id: String,
    episodes: Vec<SourceProgressMark>,
    watched: bool,
) -> Result<(), String> {
    state
        .source_progress_mark_all(&extension_id, &show_id, &episodes, watched)
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
pub fn source_history_remove(
    state: State<'_, Db>,
    extension_id: String,
    show_id: String,
    episode_id: String,
) -> Result<(), String> {
    state
        .source_history_remove(&extension_id, &show_id, &episode_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn backup_export(state: State<'_, Db>, path: String) -> Result<BackupExport, String> {
    let bytes = state.backup_to(&path).map_err(ipc_error)?;
    Ok(BackupExport { path, bytes })
}

/// Restores a validated backup. The live database is copied to a timestamped
/// safety file first, and restored extensions are left disabled until reviewed.
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
