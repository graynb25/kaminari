// Commands exposed to Svelte. Payloads in camelCase, typed, no secrets here.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::anilist::AniListClient;
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
    pub shelves: Vec<(String, Vec<String>)>,
    pub anilist_endpoint: String,
    pub limits: HostLimits,
}

#[tauri::command]
pub async fn boot_snapshot(state: State<'_, AniListClient>) -> Result<BootSnapshot, String> {
    // P0: return shape Svelte expects. Real AniList fetch lands here next.
    Ok(BootSnapshot {
        title: "Kaminari (雷)".to_string(),
        shelves: vec![("Trending".to_string(), Vec::new())],
        anilist_endpoint: state.endpoint().to_string(),
        limits: HostLimits {
            ext_timeout_secs: ext_host::TIMEOUT_SECS,
            ext_resolve_timeout_secs: ext_host::RESOLVE_TIMEOUT_SECS,
            ext_memory_bytes: ext_host::MEMORY_BYTES,
        },
    })
}

#[tauri::command]
pub fn source_validate_id(id: String) -> bool {
    ext_host::validate_source_id(&id)
}
