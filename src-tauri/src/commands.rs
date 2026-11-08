// Commands exposed to Svelte. Payloads in camelCase, typed, no secrets here.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::State;

use crate::anilist::{AniListClient, Media, ShelfKind};
use crate::db::{ContinueItem, Db, HistoryItem, InstalledSource, JsExtensionRow, LibraryItem};
use crate::error::AppError;
use crate::ext_host::{self, ExtPackage, ExtStorage};
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
