// AniList client with token bucket. Public ceiling near 90 per minute.
// Sustained 60 per minute, burst to 90. Honors Retry-After.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{AppError, AppResult};

const ENDPOINT: &str = "https://graphql.anilist.co";

pub const ANILIST_AUTH_URL: &str =
    "https://anilist.co/api/v2/oauth/authorize?client_id=56484&response_type=token";

pub fn auth_url() -> &'static str {
    ANILIST_AUTH_URL
}

/// Parses only the AniList implicit grant callback. The token stays in Rust.
pub fn parse_auth_callback(url: &Url) -> Result<String, &'static str> {
    if url.scheme() != "kaminari" || url.host_str() != Some("auth") || url.path() != "/callback" {
        return Err("not an AniList callback");
    }
    let fragment = url.fragment().ok_or("AniList callback has no token")?;
    url::form_urlencoded::parse(fragment.as_bytes())
        .find(|(key, _)| key == "access_token")
        .map(|(_, value)| value.into_owned())
        .filter(|token| !token.trim().is_empty())
        .ok_or("AniList callback has no token")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTitle {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverImage {
    pub extra_large: Option<String>,
    pub large: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub id: i64,
    pub title: MediaTitle,
    pub cover_image: Option<CoverImage>,
    pub banner_image: Option<String>,
    pub description: Option<String>,
    pub genres: Option<Vec<String>>,
    pub average_score: Option<i32>,
    pub season_year: Option<i32>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub episodes: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AniListListEntry {
    pub media: Media,
    pub status: String,
    pub progress: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewer {
    pub id: i64,
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AniListAuthStatus {
    pub connected: bool,
    pub viewer: Option<Viewer>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShelfKind {
    Trending,
    Airing,
    Top,
    Popular,
}

impl ShelfKind {
    pub fn from_key(key: &str) -> Self {
        match key {
            "airing" => Self::Airing,
            "top" => Self::Top,
            "popular" => Self::Popular,
            _ => Self::Trending,
        }
    }

    fn sort(self) -> &'static str {
        match self {
            Self::Trending => "TRENDING_DESC",
            Self::Airing => "POPULARITY_DESC",
            Self::Top => "SCORE_DESC",
            Self::Popular => "POPULARITY_DESC",
        }
    }

    fn status(self) -> Option<&'static str> {
        match self {
            Self::Airing => Some("RELEASING"),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Page {
    media: Vec<Media>,
}

#[derive(Debug, Deserialize)]
struct PageData {
    #[serde(rename = "Page")]
    page: Page,
}

#[derive(Debug, Deserialize)]
struct PageEnvelope {
    data: Option<PageData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct MediaData {
    #[serde(rename = "Media")]
    media: Media,
}

#[derive(Debug, Deserialize)]
struct MediaEnvelope {
    data: Option<MediaData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct ViewerAvatar {
    large: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ViewerResponse {
    id: i64,
    name: String,
    avatar: Option<ViewerAvatar>,
}

#[derive(Debug, Deserialize)]
struct ViewerData {
    #[serde(rename = "Viewer")]
    viewer: ViewerResponse,
}

#[derive(Debug, Deserialize)]
struct ViewerEnvelope {
    data: Option<ViewerData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct MediaListEntryResponse {
    status: String,
    #[serde(default)]
    progress: i64,
    media: Media,
}

#[derive(Debug, Deserialize)]
struct MediaListGroup {
    #[serde(default)]
    entries: Vec<MediaListEntryResponse>,
}

#[derive(Debug, Deserialize)]
struct MediaListCollection {
    #[serde(default)]
    lists: Vec<MediaListGroup>,
}

#[derive(Debug, Deserialize)]
struct MediaListData {
    #[serde(rename = "MediaListCollection")]
    collection: MediaListCollection,
}

#[derive(Debug, Deserialize)]
struct MediaListEnvelope {
    data: Option<MediaListData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct AniListClient {
    http: reqwest::Client,
    bucket: Mutex<Bucket>,
    token: Mutex<Option<String>>,
}

impl Default for AniListClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AniListClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("reqwest client"),
            bucket: Mutex::new(Bucket {
                tokens: 90.0,
                last_refill: Instant::now(),
            }),
            token: Mutex::new(None),
        }
    }

    pub fn set_token(&self, token: &str) {
        *self.token.lock().expect("token lock") = Some(token.to_string());
    }

    pub fn clear_token(&self) {
        *self.token.lock().expect("token lock") = None;
    }

    async fn acquire(&self) {
        loop {
            let wait = {
                let mut bucket = self.bucket.lock().expect("bucket lock");
                let now = Instant::now();
                let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
                bucket.tokens = (bucket.tokens + elapsed).min(90.0);
                bucket.last_refill = now;
                if bucket.tokens >= 1.0 {
                    bucket.tokens -= 1.0;
                    return;
                }
                Duration::from_secs_f64(1.0 - bucket.tokens)
            };
            tokio::time::sleep(wait).await;
        }
    }

    pub fn endpoint(&self) -> &'static str {
        ENDPOINT
    }

    async fn post(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> AppResult<serde_json::Value> {
        self.acquire().await;
        let token = self.token.lock().expect("token lock").clone();
        let mut request = self
            .http
            .post(ENDPOINT)
            .header(
                "User-Agent",
                concat!(
                    "Kaminari/",
                    env!("CARGO_PKG_VERSION"),
                    " (+https://github.com/graynb25/kaminari)"
                ),
            )
            .json(&serde_json::json!({ "query": query, "variables": variables }));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|err| AppError::Net(err.to_string()))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(60);
            return Err(AppError::RateLimited(retry_after));
        }

        if matches!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            return Err(AppError::Auth(
                "AniList rejected the saved token. Connect again.".to_string(),
            ));
        }

        if !response.status().is_success() {
            return Err(AppError::Net(format!(
                "AniList answered {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|err| AppError::Net(err.to_string()))
    }

    pub async fn trending(&self, page: u32) -> AppResult<Vec<Media>> {
        self.shelf(ShelfKind::Trending, page).await
    }

    pub async fn shelf(&self, kind: ShelfKind, page: u32) -> AppResult<Vec<Media>> {
        let status = kind.status();
        let mut variables = serde_json::json!({
            "page": page,
            "perPage": 24,
            "sort": [kind.sort()],
        });
        if let Some(status) = status {
            variables["status"] = serde_json::Value::String(status.to_string());
        }
        let json = self.post(&shelf_query(status.is_some()), variables).await?;
        let envelope: PageEnvelope =
            serde_json::from_value(json).map_err(|err| AppError::Net(err.to_string()))?;
        if let Some(errors) = envelope.errors {
            return Err(AppError::Net(graphql_message(errors)));
        }
        Ok(envelope
            .data
            .map(|data| data.page.media)
            .unwrap_or_default())
    }

    pub async fn search(&self, query: &str, page: u32) -> AppResult<Vec<Media>> {
        let json = self
            .post(
                &search_query(),
                serde_json::json!({ "search": query, "page": page, "perPage": 24 }),
            )
            .await?;
        let envelope: PageEnvelope =
            serde_json::from_value(json).map_err(|err| AppError::Net(err.to_string()))?;
        if let Some(errors) = envelope.errors {
            return Err(AppError::Net(graphql_message(errors)));
        }
        Ok(envelope
            .data
            .map(|data| data.page.media)
            .unwrap_or_default())
    }

    pub async fn detail(&self, id: i64) -> AppResult<Media> {
        let json = self
            .post(&detail_query(), serde_json::json!({ "id": id }))
            .await?;
        let envelope: MediaEnvelope =
            serde_json::from_value(json).map_err(|err| AppError::Net(err.to_string()))?;
        if let Some(errors) = envelope.errors {
            return Err(AppError::Net(graphql_message(errors)));
        }
        envelope
            .data
            .map(|data| data.media)
            .ok_or_else(|| AppError::Net("AniList returned no media".to_string()))
    }

    pub async fn viewer(&self) -> AppResult<Viewer> {
        let json = self.post(&viewer_query(), serde_json::json!({})).await?;
        let envelope: ViewerEnvelope =
            serde_json::from_value(json).map_err(|err| AppError::Net(err.to_string()))?;
        if let Some(errors) = envelope.errors {
            return Err(AppError::Auth(graphql_message(errors)));
        }
        let viewer = envelope
            .data
            .map(|data| data.viewer)
            .ok_or_else(|| AppError::Auth("AniList returned no viewer".to_string()))?;
        Ok(Viewer {
            id: viewer.id,
            name: viewer.name,
            avatar: viewer.avatar.and_then(|avatar| avatar.large),
        })
    }

    pub async fn media_list(&self, viewer_id: i64) -> AppResult<Vec<AniListListEntry>> {
        let json = self
            .post(
                &media_list_query(),
                serde_json::json!({ "userId": viewer_id }),
            )
            .await?;
        let envelope: MediaListEnvelope =
            serde_json::from_value(json).map_err(|err| AppError::Net(err.to_string()))?;
        list_entries(envelope)
    }
}

fn list_entries(envelope: MediaListEnvelope) -> AppResult<Vec<AniListListEntry>> {
    if let Some(errors) = envelope.errors {
        return Err(AppError::Net(graphql_message(errors)));
    }
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    if let Some(data) = envelope.data {
        for group in data.collection.lists {
            for entry in group.entries {
                if seen.insert(entry.media.id) {
                    entries.push(AniListListEntry {
                        media: entry.media,
                        status: entry.status,
                        progress: entry.progress,
                    });
                }
            }
        }
    }
    Ok(entries)
}

fn graphql_message(errors: Vec<GraphQlError>) -> String {
    errors
        .into_iter()
        .map(|error| error.message)
        .collect::<Vec<_>>()
        .join("; ")
}

const MEDIA_FIELDS: &str = r#"
  id
  title { romaji english native }
  coverImage { extraLarge large color }
  bannerImage
  description
  genres
  averageScore
  seasonYear
  format
  status
  episodes
"#;

fn media_query(body: &str) -> String {
    body.replace("__MEDIA_FIELDS__", MEDIA_FIELDS)
}

fn shelf_query(with_status: bool) -> String {
    let status_arg = if with_status { ", status: $status" } else { "" };
    let declaration = if with_status {
        "$page: Int, $perPage: Int, $sort: [MediaSort], $status: MediaStatus"
    } else {
        "$page: Int, $perPage: Int, $sort: [MediaSort]"
    };
    media_query(&format!(
        r#"
query Shelf({declaration}) {{
  Page(page: $page, perPage: $perPage) {{
    media(type: ANIME, sort: $sort{status_arg}) {{
      __MEDIA_FIELDS__
    }}
  }}
}}
"#
    ))
}

fn search_query() -> String {
    media_query(
        r#"
query Search($search: String, $page: Int, $perPage: Int) {
  Page(page: $page, perPage: $perPage) {
    media(type: ANIME, search: $search, sort: POPULARITY_DESC) {
      __MEDIA_FIELDS__
    }
  }
}
"#,
    )
}

fn detail_query() -> String {
    media_query(
        r#"
query Detail($id: Int) {
  Media(id: $id, type: ANIME) {
    __MEDIA_FIELDS__
  }
}
"#,
    )
}

fn viewer_query() -> String {
    r#"
query Viewer {
  Viewer {
    id
    name
    avatar { large }
  }
}
"#
    .to_string()
}

fn media_list_query() -> String {
    media_query(
        r#"
query MediaList($userId: Int!) {
  MediaListCollection(userId: $userId, type: ANIME) {
    lists {
      entries {
        status
        progress
        media {
          __MEDIA_FIELDS__
        }
      }
    }
  }
}
"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_response() {
        let raw = r##"{
          "data": {
            "Page": {
              "media": [{
                "id": 15125,
                "title": { "romaji": "Sousou no Frieren", "english": "Frieren: Beyond Journey's End", "native": "葬送のフリーレン" },
                "coverImage": { "extraLarge": "https://img.example/frieren.jpg", "large": null, "color": "#7c5cff" },
                "bannerImage": "https://img.example/frieren-banner.jpg",
                "description": "An elf mage walks on.",
                "genres": ["Adventure", "Drama"],
                "averageScore": 91,
                "seasonYear": 2023,
                "format": "TV",
                "status": "FINISHED",
                "episodes": 28
              }]
            }
          }
        }"##;
        let envelope: PageEnvelope = serde_json::from_str(raw).unwrap();
        let media = envelope.data.unwrap().page.media;
        assert_eq!(media.len(), 1);
        assert_eq!(
            media[0].title.english.as_deref(),
            Some("Frieren: Beyond Journey's End")
        );
        assert_eq!(media[0].average_score, Some(91));
        assert_eq!(media[0].id, 15125);
    }

    #[test]
    fn shelf_kinds_map_to_sorts_and_status() {
        assert_eq!(ShelfKind::from_key("trending"), ShelfKind::Trending);
        assert_eq!(ShelfKind::from_key("airing"), ShelfKind::Airing);
        assert_eq!(ShelfKind::from_key("top"), ShelfKind::Top);
        assert_eq!(ShelfKind::from_key("popular"), ShelfKind::Popular);
        assert_eq!(ShelfKind::from_key("nonsense"), ShelfKind::Trending);
        assert_eq!(ShelfKind::Airing.status(), Some("RELEASING"));
        assert_eq!(ShelfKind::Top.sort(), "SCORE_DESC");
    }

    #[test]
    fn shelf_query_omits_status_when_not_needed() {
        let without = shelf_query(false);
        let with = shelf_query(true);
        assert!(!without.contains("$status"));
        assert!(!without.contains("status:"));
        assert!(with.contains("status: $status"));
    }

    #[test]
    fn surfaces_graphql_errors() {
        let raw = r#"{ "data": null, "errors": [{ "message": "Invalid query" }] }"#;
        let envelope: PageEnvelope = serde_json::from_str(raw).unwrap();
        let message = graphql_message(envelope.errors.unwrap());
        assert_eq!(message, "Invalid query");
    }

    #[test]
    fn auth_url_has_client_id_and_token_response() {
        let url = auth_url();
        assert!(url.contains("client_id=56484"));
        assert!(url.contains("response_type=token"));
        assert!(!url.contains("redirect_uri"));
    }

    #[test]
    fn parses_valid_auth_callback() {
        let url = Url::parse("kaminari://auth/callback#access_token=abc123.def456").unwrap();
        assert_eq!(parse_auth_callback(&url).as_deref(), Ok("abc123.def456"));
    }

    #[test]
    fn rejects_wrong_scheme() {
        let url = Url::parse("https://auth/callback#access_token=abc123").unwrap();
        assert!(parse_auth_callback(&url).is_err());
    }

    #[test]
    fn rejects_wrong_path() {
        let url = Url::parse("kaminari://auth/other#access_token=abc123").unwrap();
        assert!(parse_auth_callback(&url).is_err());
    }

    #[test]
    fn rejects_callback_without_token() {
        let url = Url::parse("kaminari://auth/callback#token_type=Bearer").unwrap();
        assert!(parse_auth_callback(&url).is_err());
    }

    #[test]
    fn decodes_percent_encoded_token() {
        let url = Url::parse("kaminari://auth/callback#access_token=abc%2Fdef%3D").unwrap();
        assert_eq!(parse_auth_callback(&url).as_deref(), Ok("abc/def="));
    }

    #[test]
    fn parses_viewer_response() {
        let raw = r#"{
          "data": { "Viewer": { "id": 42, "name": "Kite", "avatar": { "large": "https://img.example/kite.jpg" } } }
        }"#;
        let envelope: ViewerEnvelope = serde_json::from_str(raw).unwrap();
        let viewer = envelope.data.unwrap().viewer;
        assert_eq!(viewer.id, 42);
        assert_eq!(viewer.name, "Kite");
        assert_eq!(
            viewer.avatar.and_then(|avatar| avatar.large).as_deref(),
            Some("https://img.example/kite.jpg")
        );
    }

    #[test]
    fn parses_media_list_collection_into_entries() {
        let raw = r##"{
          "data": {
            "MediaListCollection": {
              "lists": [{
                "entries": [{
                  "status": "CURRENT",
                  "progress": 5,
                  "media": {
                    "id": 15125,
                    "title": { "romaji": "Sousou no Frieren", "english": null, "native": null },
                    "coverImage": { "extraLarge": "https://img.example/frieren.jpg", "large": null, "color": null },
                    "bannerImage": null,
                    "description": null,
                    "genres": ["Adventure"],
                    "averageScore": 91,
                    "seasonYear": 2023,
                    "format": "TV",
                    "status": "FINISHED",
                    "episodes": 28
                  }
                }, {
                  "status": "COMPLETED",
                  "progress": 12,
                  "media": {
                    "id": 21,
                    "title": { "romaji": "One Piece", "english": null, "native": null },
                    "coverImage": null,
                    "bannerImage": null,
                    "description": null,
                    "genres": null,
                    "averageScore": null,
                    "seasonYear": null,
                    "format": "TV",
                    "status": "RELEASING",
                    "episodes": null
                  }
                }]
              }]
            }
          }
        }"##;
        let envelope: MediaListEnvelope = serde_json::from_str(raw).unwrap();
        let entries = list_entries(envelope).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].media.id, 15125);
        assert_eq!(entries[0].status, "CURRENT");
        assert_eq!(entries[0].progress, 5);
        assert_eq!(
            entries[0].media.title.romaji.as_deref(),
            Some("Sousou no Frieren")
        );
        assert_eq!(entries[1].media.id, 21);
        assert_eq!(entries[1].status, "COMPLETED");
    }
}
