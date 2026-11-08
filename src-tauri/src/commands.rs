// Commands exposed to Svelte. Payloads in camelCase, typed, no secrets here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootSnapshot {
    pub title: String,
    pub shelves: Vec<(String, Vec<String>)>,
}

#[tauri::command]
pub async fn boot_snapshot() -> Result<BootSnapshot, String> {
    // P0: return shape Svelte expects. Real AniList fetch lands here next.
    Ok(BootSnapshot {
        title: "Kaminari (雷)".to_string(),
        shelves: vec![("Trending".to_string(), Vec::new())],
    })
}

pub mod ipc {
    pub const PREFIX: &str = "kaminari/v1/";
}
