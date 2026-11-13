// AniList client with token bucket. Public ceiling near 90 per minute.
// Sustained 60 per minute, burst to 90. Honors Retry-After.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{AppError, AppResult};

const ENDPOINT: &str = "https://graphql.anilist.co";
const AUTHORIZE_URL: &str = "https://anilist.co/api/v2/oauth/authorize";
const CLIENT_ID: &str = "56484";
const AUTH_STATE_BYTES: usize = 32;

/// How long a sign in may take before its state value stops matching.
pub const AUTH_STATE_TTL: Duration = Duration::from_secs(10 * 60);

pub fn is_auth_callback(url: &Url) -> bool {
    url.scheme() == "kaminari" && url.host_str() == Some("auth") && url.path() == "/callback"
}

/// Builds the AniList authorization URL with a caller supplied state value.
pub fn authorize_url(state: &str) -> String {
    let mut url = Url::parse(AUTHORIZE_URL).expect("static AniList authorize URL");
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "token")
        .append_pair("state", state);
    url.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthCallback {
    pub token: String,
    pub state: String,
}

/// Parses only the AniList implicit grant callback. The token stays in Rust.
pub fn parse_auth_callback(url: &Url) -> Result<AuthCallback, &'static str> {
    if !is_auth_callback(url) {
        return Err("not an AniList callback");
    }
    let fragment = url.fragment().ok_or("AniList callback has no token")?;
    let mut token = None;
    let mut state = None;
    for (key, value) in url::form_urlencoded::parse(fragment.as_bytes()) {
        match key.as_ref() {
            "access_token" if token.is_none() => token = Some(value.into_owned()),
            "state" if state.is_none() => state = Some(value.into_owned()),
            _ => {}
        }
    }
    let token = token
        .filter(|token| !token.trim().is_empty())
        .ok_or("AniList callback has no token")?;
    let state = state
        .or_else(|| {
            url.query_pairs()
                .find(|(key, _)| key == "state")
                .map(|(_, value)| value.into_owned())
        })
        .filter(|state| !state.trim().is_empty())
        .ok_or("AniList callback has no state")?;
    Ok(AuthCallback { token, state })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStateError {
    /// No sign in is pending, or the state was already used.
    Missing,
    Mismatch,
    Expired,
}

struct PendingAuthState {
    value: String,
    expires_at: Instant,
}

/// Holds the one pending AniList sign in state. One use, short lived.
#[derive(Default)]
pub struct AniListAuthState {
    pending: Mutex<Option<PendingAuthState>>,
}

impl AniListAuthState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generates and stores a fresh state value, replacing any previous one.
    pub fn begin(&self) -> Result<String, String> {
        let mut bytes = [0u8; AUTH_STATE_BYTES];
        getrandom::fill(&mut bytes).map_err(|err| format!("random source failed: {err}"))?;
        let value = hex::encode(bytes);
        let mut pending = self.pending.lock().expect("auth state lock");
        *pending = Some(PendingAuthState {
            value: value.clone(),
            expires_at: Instant::now() + AUTH_STATE_TTL,
        });
        Ok(value)
    }

    /// Checks and consumes the pending state. The stored value is cleared even
    /// when it does not match, so a callback cannot be retried.
    pub fn consume(&self, candidate: &str) -> Result<(), AuthStateError> {
        self.consume_at(candidate, Instant::now())
    }

    fn consume_at(&self, candidate: &str, now: Instant) -> Result<(), AuthStateError> {
        let pending = self.pending.lock().expect("auth state lock").take();
        let Some(pending) = pending else {
            return Err(AuthStateError::Missing);
        };
        if now >= pending.expires_at {
            return Err(AuthStateError::Expired);
        }
        if !constant_time_eq(pending.value.as_bytes(), candidate.as_bytes()) {
            return Err(AuthStateError::Mismatch);
        }
        Ok(())
    }

    /// Drops any pending state, for example when the sign in page cannot open.
    pub fn clear(&self) {
        self.pending.lock().expect("auth state lock").take();
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NextAiringEpisode {
    pub airing_at: i64,
    pub episode: i64,
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
    #[serde(default)]
    pub next_airing_episode: Option<NextAiringEpisode>,
    #[serde(default)]
    pub recommendations: Option<Recommendations>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendations {
    pub nodes: Vec<RecommendationNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationNode {
    pub media_recommendation: Option<Box<Media>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AniListListEntry {
    pub media: Media,
    pub status: String,
    pub progress: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaListEntry {
    pub status: String,
    pub progress: i64,
    #[serde(default)]
    pub updated_at: i64,
}

/// One upcoming episode from AniList. Small and local only, never written
/// to the media cache.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiringScheduleEntry {
    pub anilist_id: i64,
    pub episode: i64,
    pub airing_at: i64,
}

/// AniList accepts up to 50 ids per Page request.
pub const AIRING_BATCH_SIZE: usize = 50;

/// Splits library ids into request sized batches.
pub fn airing_batches(ids: &[i64]) -> Vec<Vec<i64>> {
    ids.chunks(AIRING_BATCH_SIZE)
        .map(|chunk| chunk.to_vec())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteDecision {
    Push,
    SkipServerNewer,
    SkipMatching,
}

pub fn anilist_status(status: &str) -> Option<&'static str> {
    match status {
        "watching" => Some("CURRENT"),
        "planning" => Some("PLANNING"),
        "completed" => Some("COMPLETED"),
        "paused" => Some("PAUSED"),
        "dropped" => Some("DROPPED"),
        _ => None,
    }
}

pub fn decide_write(
    local_status: &str,
    local_progress: i64,
    local_updated_at: i64,
    server: Option<&MediaListEntry>,
) -> WriteDecision {
    let Some(server) = server else {
        return WriteDecision::Push;
    };
    if server.updated_at > local_updated_at {
        return WriteDecision::SkipServerNewer;
    }
    if anilist_status(local_status) == Some(server.status.as_str())
        && local_progress == server.progress
    {
        WriteDecision::SkipMatching
    } else {
        WriteDecision::Push
    }
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
    Upcoming,
}

impl ShelfKind {
    pub fn from_key(key: &str) -> Self {
        match key {
            "airing" => Self::Airing,
            "top" => Self::Top,
            "popular" => Self::Popular,
            "upcoming" => Self::Upcoming,
            _ => Self::Trending,
        }
    }

    fn sort(self) -> &'static str {
        match self {
            Self::Trending => "TRENDING_DESC",
            Self::Airing => "POPULARITY_DESC",
            Self::Top => "SCORE_DESC",
            Self::Popular => "POPULARITY_DESC",
            Self::Upcoming => "POPULARITY_DESC",
        }
    }

    fn status(self) -> Option<&'static str> {
        match self {
            Self::Airing => Some("RELEASING"),
            Self::Upcoming => Some("NOT_YET_RELEASED"),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Page {
    media: Vec<Media>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiringNext {
    #[serde(default)]
    airing_at: Option<i64>,
    #[serde(default)]
    episode: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiringNode {
    #[serde(default)]
    airing_at: Option<i64>,
    #[serde(default)]
    episode: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AiringConnection {
    #[serde(default)]
    nodes: Vec<AiringNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiringMedia {
    id: i64,
    #[serde(default)]
    next_airing_episode: Option<AiringNext>,
    #[serde(default)]
    airing_schedule: Option<AiringConnection>,
}

#[derive(Debug, Deserialize)]
struct AiringPage {
    #[serde(default)]
    media: Vec<AiringMedia>,
}

#[derive(Debug, Deserialize)]
struct AiringPageData {
    #[serde(rename = "Page")]
    page: AiringPage,
}

#[derive(Debug, Deserialize)]
struct AiringPageEnvelope {
    data: Option<AiringPageData>,
    errors: Option<Vec<GraphQlError>>,
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
struct MediaListEntryData {
    #[serde(rename = "MediaList")]
    entry: Option<MediaListEntry>,
}

#[derive(Debug, Deserialize)]
struct MediaListEntryEnvelope {
    data: Option<MediaListEntryData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct SavedMediaListEntryData {
    #[serde(rename = "SaveMediaListEntry")]
    entry: MediaListEntry,
}

#[derive(Debug, Deserialize)]
struct SavedMediaListEntryEnvelope {
    data: Option<SavedMediaListEntryData>,
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

    pub fn has_token(&self) -> bool {
        self.token
            .lock()
            .expect("token lock")
            .as_ref()
            .is_some_and(|token| !token.is_empty())
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

    pub async fn media_list_entry(&self, media_id: i64) -> AppResult<Option<MediaListEntry>> {
        let json = self
            .post(
                media_list_entry_query(),
                serde_json::json!({ "mediaId": media_id }),
            )
            .await?;
        let envelope: MediaListEntryEnvelope =
            serde_json::from_value(json).map_err(|err| AppError::Net(err.to_string()))?;
        if let Some(errors) = envelope.errors {
            return Err(AppError::Net(graphql_message(errors)));
        }
        Ok(envelope.data.and_then(|data| data.entry))
    }

    pub async fn save_media_list_entry(
        &self,
        media_id: i64,
        status: &str,
        progress: i64,
    ) -> AppResult<MediaListEntry> {
        let status = anilist_status(status)
            .ok_or_else(|| AppError::Blocked(format!("Unknown status: {status}")))?;
        let json = self
            .post(
                save_media_list_entry_query(),
                serde_json::json!({
                    "mediaId": media_id,
                    "status": status,
                    "progress": progress,
                }),
            )
            .await?;
        let envelope: SavedMediaListEntryEnvelope =
            serde_json::from_value(json).map_err(|err| AppError::Net(err.to_string()))?;
        if let Some(errors) = envelope.errors {
            return Err(AppError::Net(graphql_message(errors)));
        }
        envelope
            .data
            .map(|data| data.entry)
            .ok_or_else(|| AppError::Net("AniList returned no list entry".to_string()))
    }

    /// Fetches the next airing episode for library ids in batches of 50.
    /// Prefers nextAiringEpisode and falls back to the first not yet aired
    /// schedule node. Returns only entries with a usable air time.
    pub async fn airing_schedule(&self, ids: &[i64]) -> AppResult<Vec<AiringScheduleEntry>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let query = airing_query();
        let mut entries = Vec::new();
        for batch in airing_batches(ids) {
            let json = self
                .post(&query, serde_json::json!({ "ids": batch }))
                .await?;
            let envelope: AiringPageEnvelope =
                serde_json::from_value(json).map_err(|err| AppError::Net(err.to_string()))?;
            if let Some(errors) = envelope.errors {
                return Err(AppError::Net(graphql_message(errors)));
            }
            let media = envelope
                .data
                .map(|data| data.page.media)
                .unwrap_or_default();
            for item in media {
                if let Some(entry) = parse_airing_media(item) {
                    entries.push(entry);
                }
            }
        }
        Ok(entries)
    }
}

fn parse_airing_media(media: AiringMedia) -> Option<AiringScheduleEntry> {
    if let Some(next) = media.next_airing_episode {
        if let (Some(airing_at), Some(episode)) = (next.airing_at, next.episode) {
            if airing_at > 0 {
                return Some(AiringScheduleEntry {
                    anilist_id: media.id,
                    episode,
                    airing_at,
                });
            }
        }
    }
    let nodes = media
        .airing_schedule
        .map(|conn| conn.nodes)
        .unwrap_or_default();
    let node = nodes.into_iter().next()?;
    let (airing_at, episode) = (node.airing_at?, node.episode?);
    if airing_at <= 0 {
        return None;
    }
    Some(AiringScheduleEntry {
        anilist_id: media.id,
        episode,
        airing_at,
    })
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
  nextAiringEpisode { airingAt episode }
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
    recommendations(perPage: 12) {
      nodes {
        mediaRecommendation {
          __MEDIA_FIELDS__
        }
      }
    }
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

fn media_list_entry_query() -> &'static str {
    r#"
query MediaList($mediaId: Int) {
  MediaList(mediaId: $mediaId, type: ANIME) {
    status
    progress
    updatedAt
  }
}
"#
}

fn save_media_list_entry_query() -> &'static str {
    r#"
mutation SaveMediaListEntry($mediaId: Int, $status: MediaListStatus, $progress: Int) {
  SaveMediaListEntry(mediaId: $mediaId, status: $status, progress: $progress) {
    status
    progress
    updatedAt
  }
}
"#
}

fn airing_query() -> String {
    r#"
query Airing($ids: [Int]) {
  Page(page: 1, perPage: 50) {
    media(id_in: $ids, type: ANIME) {
      id
      nextAiringEpisode { airingAt episode }
      airingSchedule(notYetAired: true, perPage: 1) {
        nodes { airingAt episode }
      }
    }
  }
}
"#
    .to_string()
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
        assert_eq!(ShelfKind::from_key("upcoming"), ShelfKind::Upcoming);
        assert_eq!(ShelfKind::from_key("nonsense"), ShelfKind::Trending);
        assert_eq!(ShelfKind::Airing.status(), Some("RELEASING"));
        assert_eq!(ShelfKind::Upcoming.status(), Some("NOT_YET_RELEASED"));
        assert_eq!(ShelfKind::Top.sort(), "SCORE_DESC");
        assert_eq!(ShelfKind::Upcoming.sort(), "POPULARITY_DESC");
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
    fn detail_query_includes_recommendations() {
        let detail = detail_query();
        assert!(detail.contains("recommendations(perPage: 12)"));
        assert!(detail.contains("mediaRecommendation"));
        assert!(!shelf_query(false).contains("recommendations"));
        assert!(!search_query().contains("recommendations"));
    }

    #[test]
    fn detail_query_includes_next_airing_episode() {
        let detail = detail_query();
        assert!(detail.contains("nextAiringEpisode { airingAt episode }"));
    }

    #[test]
    fn parses_detail_next_airing_episode() {
        let raw = r#"
          {
            "data": {
              "Media": {
                "id": 15125,
                "title": { "romaji": "Frieren", "english": "Frieren", "native": null },
                "coverImage": null,
                "bannerImage": null,
                "description": null,
                "genres": ["Adventure"],
                "averageScore": 91,
                "seasonYear": 2023,
                "format": "TV",
                "status": "RELEASING",
                "episodes": 28,
                "nextAiringEpisode": { "airingAt": 1790000000, "episode": 8 }
              }
            }
          }
        "#;
        let envelope: MediaEnvelope = serde_json::from_str(raw).unwrap();
        let media = envelope.data.unwrap().media;
        assert_eq!(
            media.next_airing_episode,
            Some(NextAiringEpisode {
                airing_at: 1790000000,
                episode: 8,
            })
        );
    }

    #[test]
    fn parses_missing_next_airing_episode_as_none() {
        let raw = r#"
          {
            "data": {
              "Media": {
                "id": 15125,
                "title": { "romaji": "Frieren", "english": "Frieren", "native": null },
                "coverImage": null,
                "bannerImage": null,
                "description": null,
                "genres": ["Adventure"],
                "averageScore": 91,
                "seasonYear": 2023,
                "format": "TV",
                "status": "FINISHED",
                "episodes": 28
              }
            }
          }
        "#;
        let envelope: MediaEnvelope = serde_json::from_str(raw).unwrap();
        let media = envelope.data.unwrap().media;
        assert!(media.next_airing_episode.is_none());
    }

    #[test]
    fn parses_recommendations_without_recursion() {
        let raw = r#"
          {
            "data": {
              "Media": {
                "id": 15125,
                "title": { "romaji": "Frieren", "english": "Frieren", "native": null },
                "coverImage": null,
                "bannerImage": null,
                "description": null,
                "genres": ["Adventure"],
                "averageScore": 91,
                "seasonYear": 2023,
                "format": "TV",
                "status": "FINISHED",
                "episodes": 28,
                "recommendations": {
                  "nodes": [{
                    "mediaRecommendation": {
                      "id": 2,
                      "title": { "romaji": "Other", "english": "Other", "native": null },
                      "coverImage": null,
                      "bannerImage": null,
                      "description": null,
                      "genres": null,
                      "averageScore": null,
                      "seasonYear": null,
                      "format": "TV",
                      "status": "FINISHED",
                      "episodes": 12
                    }
                  }]
                }
              }
            }
          }
        "#;
        let envelope: MediaEnvelope = serde_json::from_str(raw).unwrap();
        let media = envelope.data.unwrap().media;
        let nodes = media.recommendations.unwrap().nodes;
        assert_eq!(nodes.len(), 1);
        let recommended = nodes[0].media_recommendation.as_ref().unwrap();
        assert_eq!(recommended.id, 2);
        assert_eq!(recommended.title.english.as_deref(), Some("Other"));
    }

    #[test]
    fn surfaces_graphql_errors() {
        let raw = r#"{ "data": null, "errors": [{ "message": "Invalid query" }] }"#;
        let envelope: PageEnvelope = serde_json::from_str(raw).unwrap();
        let message = graphql_message(envelope.errors.unwrap());
        assert_eq!(message, "Invalid query");
    }

    #[test]
    fn auth_url_has_client_id_response_type_and_state() {
        let url = Url::parse(&authorize_url("abc123")).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("anilist.co"));
        assert_eq!(url.path(), "/api/v2/oauth/authorize");
        let params: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        assert!(params.contains(&("client_id".to_string(), CLIENT_ID.to_string())));
        assert!(params.contains(&("response_type".to_string(), "token".to_string())));
        assert!(params.contains(&("state".to_string(), "abc123".to_string())));
        assert!(!params.iter().any(|(key, _)| key == "redirect_uri"));
        assert!(url.fragment().is_none());
    }

    #[test]
    fn parses_valid_auth_callback() {
        // Example token, not a credential.
        let url =
            Url::parse("kaminari://auth/callback#access_token=test-token&state=abc123").unwrap();
        let callback = parse_auth_callback(&url).unwrap();
        assert_eq!(callback.token, "test-token");
        assert_eq!(callback.state, "abc123");
    }

    #[test]
    fn accepts_state_in_query() {
        let url =
            Url::parse("kaminari://auth/callback?state=abc123#access_token=test-token").unwrap();
        assert_eq!(parse_auth_callback(&url).unwrap().state, "abc123");
    }

    #[test]
    fn rejects_wrong_scheme() {
        let url = Url::parse("https://auth/callback#access_token=abc123&state=abc123").unwrap();
        assert!(parse_auth_callback(&url).is_err());
    }

    #[test]
    fn rejects_wrong_path() {
        let url = Url::parse("kaminari://auth/other#access_token=abc123&state=abc123").unwrap();
        assert!(parse_auth_callback(&url).is_err());
    }

    #[test]
    fn rejects_callback_without_token() {
        let url = Url::parse("kaminari://auth/callback#token_type=Bearer&state=abc123").unwrap();
        assert!(parse_auth_callback(&url).is_err());
    }

    #[test]
    fn rejects_callback_without_state() {
        let url = Url::parse("kaminari://auth/callback#access_token=test-token").unwrap();
        assert!(parse_auth_callback(&url).is_err());
    }

    #[test]
    fn rejects_callback_with_empty_state() {
        let url = Url::parse("kaminari://auth/callback#access_token=test-token&state=").unwrap();
        assert!(parse_auth_callback(&url).is_err());
    }

    #[test]
    fn decodes_percent_encoded_token_and_state() {
        let url = Url::parse("kaminari://auth/callback#access_token=abc%2Fdef%3D&state=st%2Fate")
            .unwrap();
        let callback = parse_auth_callback(&url).unwrap();
        assert_eq!(callback.token, "abc/def=");
        assert_eq!(callback.state, "st/ate");
    }

    #[test]
    fn state_is_random_and_one_time() {
        let store = AniListAuthState::new();
        let first = store.begin().unwrap();
        let second = store.begin().unwrap();
        assert_eq!(first.len(), AUTH_STATE_BYTES * 2);
        assert!(first.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(first, second);
        assert_eq!(store.consume(&second), Ok(()));
        assert_eq!(store.consume(&second), Err(AuthStateError::Missing));
    }

    #[test]
    fn missing_state_is_rejected() {
        let store = AniListAuthState::new();
        assert_eq!(store.consume("anything"), Err(AuthStateError::Missing));
    }

    #[test]
    fn mismatched_state_is_rejected_and_consumed() {
        let store = AniListAuthState::new();
        let state = store.begin().unwrap();
        assert_eq!(store.consume("wrong"), Err(AuthStateError::Mismatch));
        assert_eq!(store.consume(&state), Err(AuthStateError::Missing));
    }

    #[test]
    fn expired_state_is_rejected_and_consumed() {
        let store = AniListAuthState::new();
        let state = store.begin().unwrap();
        let later = Instant::now() + AUTH_STATE_TTL + Duration::from_secs(1);
        assert_eq!(
            store.consume_at(&state, later),
            Err(AuthStateError::Expired)
        );
        assert_eq!(store.consume(&state), Err(AuthStateError::Missing));
    }

    #[test]
    fn beginning_a_new_sign_in_replaces_the_old_state() {
        let store = AniListAuthState::new();
        let old = store.begin().unwrap();
        let new = store.begin().unwrap();
        assert_eq!(store.consume(&old), Err(AuthStateError::Mismatch));
        assert_eq!(store.consume(&new), Err(AuthStateError::Missing));
    }

    #[test]
    fn clearing_state_drops_the_pending_sign_in() {
        let store = AniListAuthState::new();
        let state = store.begin().unwrap();
        store.clear();
        assert_eq!(store.consume(&state), Err(AuthStateError::Missing));
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

    #[test]
    fn parses_media_list_entry_response() {
        let raw = r#"{
          "data": {
            "MediaList": {
              "status": "CURRENT",
              "progress": 7,
              "updatedAt": 1700000000
            }
          }
        }"#;
        let envelope: MediaListEntryEnvelope = serde_json::from_str(raw).unwrap();
        assert!(envelope.errors.is_none());
        let entry = envelope.data.unwrap().entry.unwrap();
        assert_eq!(entry.status, "CURRENT");
        assert_eq!(entry.progress, 7);
        assert_eq!(entry.updated_at, 1700000000);
    }

    #[test]
    fn parses_missing_media_list_entry_as_none() {
        let raw = r#"{ "data": { "MediaList": null } }"#;
        let envelope: MediaListEntryEnvelope = serde_json::from_str(raw).unwrap();
        assert!(envelope.data.unwrap().entry.is_none());
    }

    #[test]
    fn parses_save_media_list_entry_response() {
        let raw = r#"{
          "data": {
            "SaveMediaListEntry": {
              "status": "COMPLETED",
              "progress": 28,
              "updatedAt": 1700000010
            }
          }
        }"#;
        let envelope: SavedMediaListEntryEnvelope = serde_json::from_str(raw).unwrap();
        assert!(envelope.errors.is_none());
        let entry = envelope.data.unwrap().entry;
        assert_eq!(entry.status, "COMPLETED");
        assert_eq!(entry.progress, 28);
        assert_eq!(entry.updated_at, 1700000010);
    }

    #[test]
    fn local_statuses_map_to_anilist_values() {
        assert_eq!(anilist_status("watching"), Some("CURRENT"));
        assert_eq!(anilist_status("planning"), Some("PLANNING"));
        assert_eq!(anilist_status("completed"), Some("COMPLETED"));
        assert_eq!(anilist_status("paused"), Some("PAUSED"));
        assert_eq!(anilist_status("dropped"), Some("DROPPED"));
        assert_eq!(anilist_status("bogus"), None);
    }

    #[test]
    fn conflict_rule_skips_a_newer_server_entry() {
        let server = MediaListEntry {
            status: "CURRENT".to_string(),
            progress: 9,
            updated_at: 200,
        };
        assert_eq!(
            decide_write("watching", 3, 100, Some(&server)),
            WriteDecision::SkipServerNewer
        );
    }

    #[test]
    fn conflict_rule_skips_a_matching_entry() {
        let server = MediaListEntry {
            status: "COMPLETED".to_string(),
            progress: 12,
            updated_at: 100,
        };
        assert_eq!(
            decide_write("completed", 12, 100, Some(&server)),
            WriteDecision::SkipMatching
        );
        assert_eq!(
            decide_write("completed", 12, 200, Some(&server)),
            WriteDecision::SkipMatching
        );
    }

    #[test]
    fn conflict_rule_pushes_when_server_is_missing_or_older() {
        assert_eq!(decide_write("watching", 3, 100, None), WriteDecision::Push);

        let older = MediaListEntry {
            status: "CURRENT".to_string(),
            progress: 1,
            updated_at: 50,
        };
        assert_eq!(
            decide_write("watching", 3, 100, Some(&older)),
            WriteDecision::Push
        );

        let same_time_different = MediaListEntry {
            status: "PAUSED".to_string(),
            progress: 1,
            updated_at: 100,
        };
        assert_eq!(
            decide_write("watching", 3, 100, Some(&same_time_different)),
            WriteDecision::Push
        );
    }

    fn airing_envelope_for(media_json: &str) -> AiringPageEnvelope {
        let raw = format!(r#"{{ "data": {{ "Page": {{ "media": [{media_json}] }} }} }}"#);
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn airing_prefers_next_episode_over_schedule() {
        let envelope = airing_envelope_for(
            r#"{
              "id": 21,
              "nextAiringEpisode": { "airingAt": 1790000000, "episode": 8 },
              "airingSchedule": { "nodes": [{ "airingAt": 1790000000, "episode": 7 }] }
            }"#,
        );
        let media = envelope.data.unwrap().page.media;
        assert_eq!(media.len(), 1);
        let entry = parse_airing_media(media.into_iter().next().unwrap()).unwrap();
        assert_eq!(
            entry,
            AiringScheduleEntry {
                anilist_id: 21,
                episode: 8,
                airing_at: 1790000000,
            }
        );
    }

    #[test]
    fn airing_falls_back_to_schedule_node() {
        let envelope = airing_envelope_for(
            r#"{
              "id": 22,
              "nextAiringEpisode": null,
              "airingSchedule": { "nodes": [{ "airingAt": 1790003600, "episode": 5 }] }
            }"#,
        );
        let media = envelope.data.unwrap().page.media;
        let entry = parse_airing_media(media.into_iter().next().unwrap()).unwrap();
        assert_eq!(entry.anilist_id, 22);
        assert_eq!(entry.episode, 5);
        assert_eq!(entry.airing_at, 1790003600);
    }

    #[test]
    fn airing_supports_both_sources_together() {
        let envelope = airing_envelope_for(
            r#"{
              "id": 23,
              "nextAiringEpisode": { "airingAt": 1790010000, "episode": 12 },
              "airingSchedule": { "nodes": [{ "airingAt": 1790010000, "episode": 12 }] }
            }"#,
        );
        let media = envelope.data.unwrap().page.media;
        let entry = parse_airing_media(media.into_iter().next().unwrap()).unwrap();
        assert_eq!(entry.anilist_id, 23);
        assert_eq!(entry.episode, 12);
        assert_eq!(entry.airing_at, 1790010000);
    }

    #[test]
    fn airing_returns_none_without_either_source() {
        let envelope = airing_envelope_for(
            r#"{
              "id": 24,
              "nextAiringEpisode": null,
              "airingSchedule": { "nodes": [] }
            }"#,
        );
        let media = envelope.data.unwrap().page.media;
        assert!(parse_airing_media(media.into_iter().next().unwrap()).is_none());

        let missing = airing_envelope_for(r#"{"id": 25}"#);
        let media = missing.data.unwrap().page.media;
        assert!(parse_airing_media(media.into_iter().next().unwrap()).is_none());
    }

    #[test]
    fn airing_batches_split_at_fifty() {
        assert!(airing_batches(&[]).is_empty());

        let one: Vec<i64> = vec![7];
        let batches = airing_batches(&one);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0], vec![7]);

        let fifty: Vec<i64> = (1..=50).collect();
        let batches = airing_batches(&fifty);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 50);

        let fifty_one: Vec<i64> = (1..=51).collect();
        let batches = airing_batches(&fifty_one);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 50);
        assert_eq!(batches[1], vec![51]);
    }

    #[test]
    fn airing_query_uses_id_in_and_fallback() {
        let query = airing_query();
        assert!(query.contains("id_in: $ids"));
        assert!(query.contains("nextAiringEpisode"));
        assert!(query.contains("airingSchedule(notYetAired: true, perPage: 1)"));
        assert!(query.contains("perPage: 50"));
    }
}
