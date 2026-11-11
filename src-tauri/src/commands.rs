// Commands exposed to Svelte. Payloads in camelCase, typed, no secrets here.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::DialogExt;
use tokio::sync::Semaphore;

use crate::anilist::{
    decide_write, AniListAuthStatus, AniListClient, Media, ShelfKind, Viewer, WriteDecision,
};
use crate::db::{
    validate_source_library_input, validate_source_progress_input, validate_source_progress_marks,
    ContinueItem, Db, ExtensionInstallPreview, HistoryItem, InstalledSource, JsExtensionRow,
    LibraryItem, SourceContinueItem, SourceHistoryItem, SourceLibraryItem, SourceProgress,
    SourceProgressMark,
};
use crate::error::AppError;
use crate::ext_host::{self, ExtPackage, ExtStorage, ExtensionTrust};
use crate::mangayomi;
use crate::path_guard::{PathAuthority, PathOperation};
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AniListOAuthStatus {
    pub portable: bool,
    pub callback_registered: bool,
    pub can_register: bool,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInstallOutcome {
    pub installed: bool,
    pub approval_token: Option<String>,
    pub preview: ExtensionInstallPreview,
}

struct PendingExtensionInstall {
    token: String,
    expires_at: Instant,
    package: ExtPackage,
    trust: ExtensionTrust,
    preview: ExtensionInstallPreview,
}

/// Holds one package between its review and the user's approval. The webview
/// never gets the code or the path back, only a one-time token.
#[derive(Clone, Default)]
pub struct ExtensionInstallState {
    pending: Arc<Mutex<Option<PendingExtensionInstall>>>,
}

impl ExtensionInstallState {
    pub fn new() -> Self {
        Self::default()
    }

    fn put(
        &self,
        package: ExtPackage,
        trust: ExtensionTrust,
        preview: ExtensionInstallPreview,
    ) -> Result<String, String> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|err| format!("random source failed: {err}"))?;
        let token = hex::encode(bytes);
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "extension install lock is poisoned".to_string())?;
        *pending = Some(PendingExtensionInstall {
            token: token.clone(),
            expires_at: Instant::now() + EXTENSION_APPROVAL_TTL,
            package,
            trust,
            preview,
        });
        Ok(token)
    }

    fn take(
        &self,
        token: &str,
    ) -> Result<(ExtPackage, ExtensionTrust, ExtensionInstallPreview), String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "extension install lock is poisoned".to_string())?;
        let Some(entry) = pending.take() else {
            return Err("the update approval expired; choose the package again".to_string());
        };
        if Instant::now() >= entry.expires_at {
            return Err("the update approval expired; choose the package again".to_string());
        }
        if entry.token != token {
            return Err("the update approval is not valid; choose the package again".to_string());
        }
        Ok((entry.package, entry.trust, entry.preview))
    }
}

/// One AniList sync at a time. A manual push or pull waits for the guard; a
/// background tick skips instead of queueing behind a long run.
#[derive(Clone)]
pub struct AniListSyncGuard {
    sync: Arc<tokio::sync::Mutex<()>>,
}

impl Default for AniListSyncGuard {
    fn default() -> Self {
        Self {
            sync: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

impl AniListSyncGuard {
    pub fn new() -> Self {
        Self::default()
    }

    async fn acquire(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.sync.clone().lock_owned().await
    }

    fn try_acquire(&self) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        self.sync.clone().try_lock_owned().ok()
    }
}

const SETTINGS_MAX_BYTES: usize = 64 * 1024;
const ANILIST_KEYRING_SERVICE: &str = "app.kaminari.desktop";
const ANILIST_KEYRING_USER: &str = "anilist";
const ANILIST_SYNC_TICK_ROWS: usize = 20;
const ANILIST_SYNC_MANUAL_ROWS: usize = 100;
const EXTENSION_APPROVAL_TTL: Duration = Duration::from_secs(5 * 60);
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
pub fn anilist_oauth_status(app: AppHandle) -> AniListOAuthStatus {
    oauth_status(&app)
}

fn oauth_status(app: &AppHandle) -> AniListOAuthStatus {
    let portable = crate::portable::is_portable();
    #[cfg(any(windows, target_os = "linux"))]
    {
        match app.deep_link().is_registered("kaminari") {
            Ok(true) => AniListOAuthStatus {
                portable,
                callback_registered: true,
                can_register: true,
                message: None,
            },
            Ok(false) => AniListOAuthStatus {
                portable,
                callback_registered: false,
                can_register: true,
                message: Some(if portable {
                    "This portable build is not registered for the kaminari:// sign in callback. Enable it once for this Windows account.".to_string()
                } else {
                    "The kaminari:// sign in callback is not registered. Enable it to connect AniList.".to_string()
                }),
            },
            Err(err) => AniListOAuthStatus {
                portable,
                callback_registered: false,
                can_register: true,
                message: Some(format!(
                    "Kaminari could not check the kaminari:// callback registration: {err}"
                )),
            },
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = app;
        AniListOAuthStatus {
            portable,
            callback_registered: true,
            can_register: false,
            message: None,
        }
    }
}

#[tauri::command]
pub fn anilist_register_callback(app: AppHandle) -> Result<(), String> {
    app.deep_link()
        .register_all()
        .map_err(|err| format!("cannot register the kaminari:// callback: {err}"))?;
    #[cfg(any(windows, target_os = "linux"))]
    if !app
        .deep_link()
        .is_registered("kaminari")
        .map_err(|err| format!("cannot verify the kaminari:// callback: {err}"))?
    {
        return Err(
            "This system still does not point kaminari:// at this build. Install Kaminari normally, or allow protocol registration when the system asks."
                .to_string(),
        );
    }
    Ok(())
}

#[tauri::command]
pub fn anilist_auth_start(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;

    #[cfg(any(windows, target_os = "linux"))]
    {
        let registered = app
            .deep_link()
            .is_registered("kaminari")
            .map_err(|err| format!("cannot check the kaminari:// callback: {err}"))?;
        if !registered {
            return Err(
                "Enable the kaminari:// sign in callback first, then connect AniList.".to_string(),
            );
        }
    }

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
    guard: State<'_, AniListSyncGuard>,
) -> Result<AniListSyncResult, String> {
    let _guard = guard.acquire().await;
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

/// Pushes at most `limit` due rows. The queue snapshot is re-checked before
/// every write and every delete, so a local edit made while a request is in
/// flight is never overwritten or dropped.
async fn push_pending_anilist(
    client: &AniListClient,
    db: &Db,
    limit: usize,
) -> Result<AniListPushResult, AppError> {
    let rows = db.pending_anilist_sync_due(limit)?;
    let mut pushed = 0;
    let mut conflicts = 0;
    for row in rows {
        if !client.has_token() {
            break;
        }
        let Some(current) = db.anilist_sync_row(row.anilist_id)? else {
            continue;
        };
        if current.updated_at != row.updated_at
            || current.attempts != row.attempts
            || current.status != row.status
            || current.progress != row.progress
        {
            continue;
        }
        let server = match client.media_list_entry(row.anilist_id).await {
            Ok(entry) => entry,
            Err(err @ (AppError::Net(_) | AppError::RateLimited(_))) => {
                db.fail_anilist_sync_snapshot(&row, &err.to_string())?;
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
                if db.complete_anilist_sync(&row)? {
                    conflicts += 1;
                }
            }
            WriteDecision::SkipMatching => {
                db.complete_anilist_sync(&row)?;
            }
            WriteDecision::Push => {
                match client
                    .save_media_list_entry(row.anilist_id, &row.status, row.progress)
                    .await
                {
                    Ok(_) => {
                        if db.complete_anilist_sync(&row)? {
                            pushed += 1;
                        }
                    }
                    Err(err @ (AppError::Net(_) | AppError::RateLimited(_))) => {
                        db.fail_anilist_sync_snapshot(&row, &err.to_string())?;
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
    guard: State<'_, AniListSyncGuard>,
) -> Result<AniListPushResult, String> {
    let _guard = guard.acquire().await;
    let token = load_anilist_token()?
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| "Connect AniList first.".to_string())?;
    state.set_token(&token);
    let result = push_pending_anilist(state.inner(), db.inner(), ANILIST_SYNC_MANUAL_ROWS)
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
    let guard = app.state::<AniListSyncGuard>();
    let Some(_guard) = guard.try_acquire() else {
        tracing::debug!("anilist: previous sync is still running, skipping this tick");
        return;
    };
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
    match push_pending_anilist(client.inner(), db.inner(), ANILIST_SYNC_TICK_ROWS).await {
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
    // The sample is compiled into this build, so its trust and content do not
    // come from a package file. Reinstalling it is safe without a prompt.
    state
        .replace_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
        .map_err(ipc_error)
}

fn install_extension_or_request_approval(
    db: &Db,
    installs: &ExtensionInstallState,
    package: ExtPackage,
    trust: ExtensionTrust,
) -> Result<ExtensionInstallOutcome, String> {
    let preview = db
        .extension_install_preview(&package.manifest, &package.code, trust)
        .map_err(ipc_error)?;
    if preview.requires_approval {
        let approval_token = installs.put(package, trust, preview.clone())?;
        return Ok(ExtensionInstallOutcome {
            installed: false,
            approval_token: Some(approval_token),
            preview,
        });
    }
    db.install_js_extension(&package.manifest, &package.code, trust)
        .map_err(ipc_error)?;
    Ok(ExtensionInstallOutcome {
        installed: true,
        approval_token: None,
        preview,
    })
}

#[tauri::command]
pub async fn js_extension_install_pick(
    app: AppHandle,
    authority: State<'_, PathAuthority>,
) -> Result<Option<String>, String> {
    let Some(path) = run_file_dialog(
        app,
        FileDialogMode::Open,
        "Install extension package",
        &["kext"],
        "package.kext",
    )
    .await?
    else {
        return Ok(None);
    };
    authority
        .grant(&path, PathOperation::ExtensionInstall)
        .map(Some)
}

#[tauri::command]
pub async fn js_extension_install_package(
    authority: State<'_, PathAuthority>,
    state: State<'_, Db>,
    installs: State<'_, ExtensionInstallState>,
    token: String,
) -> Result<ExtensionInstallOutcome, String> {
    let path = authority.consume(&token, PathOperation::ExtensionInstall)?;
    let db = state.inner().clone();
    let installs = installs.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let metadata = std::fs::metadata(&path).map_err(|err| {
            AppError::Extension(format!("cannot read package: {err}")).to_string()
        })?;
        if metadata.len() > ext_host::MAX_KEXT_BYTES as u64 {
            return Err("the package is larger than 4 MB".to_string());
        }
        let bytes = std::fs::read(&path).map_err(|err| {
            AppError::Extension(format!("cannot read package: {err}")).to_string()
        })?;
        if bytes.len() > ext_host::MAX_KEXT_BYTES {
            return Err("the package is larger than 4 MB".to_string());
        }
        let package = ext_host::package_from_kext(&bytes).map_err(ipc_error)?;
        install_extension_or_request_approval(&db, &installs, package, ExtensionTrust::Sideloaded)
    })
    .await
    .map_err(|err| format!("extension install failed: {err}"))?
}

#[tauri::command]
pub fn js_extension_install_confirm(
    state: State<'_, Db>,
    installs: State<'_, ExtensionInstallState>,
    approval_token: String,
) -> Result<(), String> {
    let (package, trust, expected) = installs.take(&approval_token)?;
    let current = state
        .extension_install_preview(&package.manifest, &package.code, trust)
        .map_err(ipc_error)?;
    if current != expected {
        return Err(
            "the installed extension changed while the update was being reviewed; choose the package again"
                .to_string(),
        );
    }
    state
        .replace_js_extension(&package.manifest, &package.code, trust)
        .map_err(ipc_error)
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
    installs: State<'_, ExtensionInstallState>,
    url: String,
    source_index: usize,
    sha256: String,
) -> Result<ExtensionInstallOutcome, String> {
    let (code, actual, _) = fetch_mangayomi_source(&url).await?;
    if actual != sha256 {
        return Err(
            "the source changed since inspection; inspect it again before installing".to_string(),
        );
    }
    let source = mangayomi::source_at(&code, source_index).map_err(ipc_error)?;
    let manifest = mangayomi::build_manifest(&source, source_index).map_err(ipc_error)?;
    let wrapped = mangayomi::wrap_code(&code, source_index);
    let package = ExtPackage {
        manifest,
        code: wrapped,
    };
    install_extension_or_request_approval(
        state.inner(),
        installs.inner(),
        package,
        ExtensionTrust::Sideloaded,
    )
}

async fn fetch_mangayomi_source(url: &str) -> Result<(String, String, usize), String> {
    let parsed =
        reqwest::Url::parse(url.trim()).map_err(|_| "source URL is not valid".to_string())?;
    crate::safe_http::validate_https_url(&parsed)?;
    let client =
        crate::safe_http::SafeFetch::new("Kaminari", Some(Duration::from_secs(30)), false)?;
    let response = client
        .get(parsed.as_str(), |_url, request| {
            request.header(reqwest::header::ACCEPT, "text/javascript, text/plain, */*")
        })
        .await
        .map_err(|err| format!("source download failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "source download failed with HTTP {}",
            response.status()
        ));
    }
    let body = crate::safe_http::read_capped(response, mangayomi::MAX_SOURCE_BYTES, "source")
        .await
        .map_err(|err| {
            if err.contains("larger than") {
                "source is larger than 2 MB".to_string()
            } else {
                format!("source download failed: {err}")
            }
        })?;
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
    validate_source_library_input(
        &extension_id,
        &show_id,
        &title,
        cover.as_deref(),
        status.as_deref(),
    )
    .map_err(ipc_error)?;
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
    validate_source_progress_input(
        &extension_id,
        &show_id,
        &episode_id,
        episode_number,
        episode_title.as_deref(),
        position_sec,
        duration_sec,
    )
    .map_err(ipc_error)?;
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
    validate_source_progress_marks(&extension_id, &show_id, &episodes).map_err(ipc_error)?;
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
pub async fn backup_export_pick(
    app: AppHandle,
    authority: State<'_, PathAuthority>,
) -> Result<Option<String>, String> {
    let Some(path) = run_file_dialog(
        app,
        FileDialogMode::Save,
        "Backup library",
        &["db"],
        "kaminari-backup.db",
    )
    .await?
    else {
        return Ok(None);
    };
    authority
        .grant(&path, PathOperation::BackupExport)
        .map(Some)
}

#[tauri::command]
pub async fn backup_export(
    authority: State<'_, PathAuthority>,
    state: State<'_, Db>,
    token: String,
) -> Result<BackupExport, String> {
    let path = authority.consume(&token, PathOperation::BackupExport)?;
    let db = state.inner().clone();
    let bytes =
        tauri::async_runtime::spawn_blocking(move || db.backup_to(path.to_string_lossy().as_ref()))
            .await
            .map_err(|err| format!("backup failed: {err}"))?
            .map_err(ipc_error)?;
    Ok(BackupExport { bytes })
}

/// Restores a validated backup. The live database is copied to a timestamped
/// safety file first, and restored extensions are left disabled until reviewed.
#[tauri::command]
pub async fn backup_restore_pick(
    app: AppHandle,
    authority: State<'_, PathAuthority>,
) -> Result<Option<String>, String> {
    let Some(path) = run_file_dialog(
        app,
        FileDialogMode::Open,
        "Restore backup",
        &["db"],
        "kaminari-backup.db",
    )
    .await?
    else {
        return Ok(None);
    };
    authority
        .grant(&path, PathOperation::BackupRestore)
        .map(Some)
}

#[tauri::command]
pub async fn backup_restore(
    authority: State<'_, PathAuthority>,
    state: State<'_, Db>,
    token: String,
) -> Result<(), String> {
    let path = authority.consume(&token, PathOperation::BackupRestore)?;
    let db = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        db.restore_from(path.to_string_lossy().as_ref())
            .map_err(ipc_error)
    })
    .await
    .map_err(|err| format!("restore failed: {err}"))?
}

#[tauri::command]
pub async fn settings_export_pick(
    app: AppHandle,
    authority: State<'_, PathAuthority>,
) -> Result<Option<String>, String> {
    let Some(path) = run_file_dialog(
        app,
        FileDialogMode::Save,
        "Export settings",
        &["json"],
        "kaminari-settings.json",
    )
    .await?
    else {
        return Ok(None);
    };
    authority
        .grant(&path, PathOperation::SettingsExport)
        .map(Some)
}

#[tauri::command]
pub async fn settings_export(
    authority: State<'_, PathAuthority>,
    token: String,
    json: String,
) -> Result<(), String> {
    if json.len() > SETTINGS_MAX_BYTES {
        return Err("settings file is too large (64 KiB limit)".to_string());
    }
    let target = authority.consume(&token, PathOperation::SettingsExport)?;
    tauri::async_runtime::spawn_blocking(move || write_atomic(&target, json.as_bytes()))
        .await
        .map_err(|err| format!("settings export failed: {err}"))?
}

#[tauri::command]
pub async fn settings_import_pick(
    app: AppHandle,
    authority: State<'_, PathAuthority>,
) -> Result<Option<String>, String> {
    let Some(path) = run_file_dialog(
        app,
        FileDialogMode::Open,
        "Import settings",
        &["json"],
        "kaminari-settings.json",
    )
    .await?
    else {
        return Ok(None);
    };
    authority
        .grant(&path, PathOperation::SettingsImport)
        .map(Some)
}

#[tauri::command]
pub async fn settings_read(
    authority: State<'_, PathAuthority>,
    token: String,
) -> Result<String, String> {
    let target = authority.consume(&token, PathOperation::SettingsImport)?;
    tauri::async_runtime::spawn_blocking(move || read_settings_file(&target))
        .await
        .map_err(|err| format!("settings import failed: {err}"))?
}

#[tauri::command]
pub fn app_restart(app: AppHandle) {
    app.restart();
}

#[derive(Clone, Copy)]
enum FileDialogMode {
    Open,
    Save,
}

async fn run_file_dialog(
    app: AppHandle,
    mode: FileDialogMode,
    title: &'static str,
    extensions: &'static [&'static str],
    default_name: &'static str,
) -> Result<Option<PathBuf>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = app
            .dialog()
            .file()
            .set_title(title)
            .add_filter("Kaminari file", extensions);
        let picked = match mode {
            FileDialogMode::Open => dialog.blocking_pick_file(),
            FileDialogMode::Save => dialog.set_file_name(default_name).blocking_save_file(),
        };
        picked
            .map(|file| {
                file.simplified()
                    .into_path()
                    .map_err(|err| format!("the selected path is not valid: {err}"))
            })
            .transpose()
    })
    .await
    .map_err(|err| format!("file dialog failed: {err}"))?
}

fn read_settings_file(target: &Path) -> Result<String, String> {
    let metadata =
        std::fs::metadata(target).map_err(|err| format!("cannot read settings file: {err}"))?;
    if metadata.len() > SETTINGS_MAX_BYTES as u64 {
        return Err("settings file is too large (64 KiB limit)".to_string());
    }
    let bytes = std::fs::read(target).map_err(|err| format!("cannot read settings file: {err}"))?;
    if bytes.len() > SETTINGS_MAX_BYTES {
        return Err("settings file is too large (64 KiB limit)".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "settings file is not UTF-8".to_string())
}

fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| "settings path has no file name".to_string())?;
    let mut created = None;
    for _ in 0..8 {
        let temp = target.with_file_name(format!(".{name}.kaminari-{}.tmp", random_suffix()?));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(file) => {
                created = Some((temp, file));
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("cannot write settings file: {err}")),
        }
    }
    let Some((temp, mut file)) = created else {
        return Err("cannot create a temporary settings file".to_string());
    };
    let result = (|| -> std::io::Result<()> {
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

fn random_suffix() -> Result<String, String> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|err| format!("random source failed: {err}"))?;
    Ok(hex::encode(bytes))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_running_sync_makes_the_next_tick_skip() {
        let guard = AniListSyncGuard::new();
        let held = guard.acquire().await;
        assert!(guard.try_acquire().is_none());
        drop(held);
        assert!(guard.try_acquire().is_some());
    }

    #[test]
    fn approval_tokens_are_one_use() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        let preview = db
            .extension_install_preview(&package.manifest, &package.code, ExtensionTrust::Sideloaded)
            .unwrap();
        let installs = ExtensionInstallState::new();
        let token = installs
            .put(package, ExtensionTrust::Sideloaded, preview)
            .unwrap();
        let (package, trust, taken_preview) = installs.take(&token).unwrap();
        assert_eq!(trust, ExtensionTrust::Sideloaded);
        assert!(package.manifest.id.starts_with("app.kaminari."));
        assert!(taken_preview.new_install);
        assert!(installs.take(&token).is_err());
    }

    #[test]
    fn an_unknown_approval_token_is_rejected() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        let preview = db
            .extension_install_preview(&package.manifest, &package.code, ExtensionTrust::Sideloaded)
            .unwrap();
        let installs = ExtensionInstallState::new();
        let _token = installs
            .put(package, ExtensionTrust::Sideloaded, preview)
            .unwrap();
        assert!(installs.take("not-the-token").is_err());
        // The wrong attempt consumes the pending package; a fresh install is
        // needed, which is the safe failure direction.
        assert!(installs.take("not-the-token").is_err());
    }
}
