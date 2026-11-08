// Commands exposed to Svelte. Payloads in camelCase, typed, no secrets here.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::anilist::{AniListClient, Media, ShelfKind};
use crate::db::{ContinueItem, Db, LibraryItem};
use crate::error::AppError;
use crate::ext_host;

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
pub fn source_validate_id(id: String) -> bool {
    ext_host::validate_source_id(&id)
}
