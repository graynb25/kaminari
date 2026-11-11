// Aniyomi style registry reader. Metadata only. Nothing here executes an APK.

use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{AppError, AppResult};
use crate::safe_http::{self, SafeFetch};

const MAX_BYTES: usize = 20 * 1024 * 1024;
const MAX_EXTENSIONS: usize = 500;
const USER_AGENT_TEXT: &str = concat!(
    "Kaminari/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/graynb25/kaminari)"
);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacySource {
    pub name: String,
    pub lang: String,
    pub id: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyExtension {
    pub name: String,
    pub pkg: String,
    pub apk: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub code: u32,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub nsfw: u8,
    #[serde(default)]
    pub torrent: u8,
    #[serde(default)]
    pub sources: Vec<LegacySource>,
}

impl LegacyExtension {
    pub fn valid(&self) -> bool {
        !self.pkg.trim().is_empty()
            && !self.name.trim().is_empty()
            && self.sources.len() <= 64
            && self.sources.iter().all(|source| {
                !source.id.is_empty() && source.id.chars().all(|c| c.is_ascii_digit())
            })
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn is_nsfw(&self) -> bool {
        self.nsfw > 0
    }
}

pub async fn fetch_index(url: &str) -> AppResult<Vec<LegacyExtension>> {
    validate_url(url)?;

    let client = SafeFetch::new(
        USER_AGENT_TEXT,
        Some(std::time::Duration::from_secs(15)),
        false,
    )
    .map_err(AppError::Net)?;

    let response = client
        .get(url, |_url, request| {
            request.header(ACCEPT, "application/json")
        })
        .await
        .map_err(AppError::Net)?;

    if !response.status().is_success() {
        return Err(AppError::Net(format!(
            "Registry answered {}",
            response.status()
        )));
    }

    let bytes = safe_http::read_capped(response, MAX_BYTES, "registry")
        .await
        .map_err(AppError::Net)?;

    let mut entries: Vec<LegacyExtension> = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Net(format!("Registry JSON is invalid: {err}")))?;

    entries.retain(LegacyExtension::valid);
    entries.truncate(MAX_EXTENSIONS);
    Ok(entries)
}

fn validate_url(url: &str) -> AppResult<()> {
    let parsed = Url::parse(url.trim())
        .map_err(|_| AppError::Blocked("Registry URL is not valid".to_string()))?;
    safe_http::validate_https_url(&parsed).map_err(AppError::Blocked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_digit_only_source_ids() {
        let raw = r#"[
          {
            "name": "Aniyomi: AnimeOnsen",
            "pkg": "eu.kanade.tachiyomi.animeextension.all.animeonsen",
            "apk": "aniyomi-all.animeonsen-v14.10.apk",
            "lang": "all",
            "code": 10,
            "version": "14.10",
            "nsfw": 0,
            "torrent": 0,
            "sources": [
              { "name": "AnimeOnsen", "lang": "all", "id": "8542735178285060053", "baseUrl": "https://www.animeonsen.xyz" }
            ]
          }
        ]"#;
        let entries: Vec<LegacyExtension> = serde_json::from_str(raw).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].valid());
        assert_eq!(entries[0].source_count(), 1);
        assert_eq!(entries[0].sources[0].id, "8542735178285060053");
    }

    #[test]
    fn rejects_non_digit_source_ids_and_http() {
        let bad = LegacyExtension {
            name: "Bad".to_string(),
            pkg: "com.example.bad".to_string(),
            apk: "bad.apk".to_string(),
            lang: "en".to_string(),
            code: 1,
            version: "1.0".to_string(),
            nsfw: 0,
            torrent: 0,
            sources: vec![LegacySource {
                name: "Bad".to_string(),
                lang: "en".to_string(),
                id: "not-a-number".to_string(),
                base_url: "https://example.com".to_string(),
            }],
        };
        assert!(!bad.valid());
        assert!(validate_url("http://example.com").is_err());
        assert!(validate_url("https://example.com/index.json").is_ok());
    }
}
