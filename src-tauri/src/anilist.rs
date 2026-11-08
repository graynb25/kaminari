// AniList client with token bucket. Public ceiling near 90 per minute.
// Sustained 60 per minute, burst to 90. Honors Retry-After.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const ENDPOINT: &str = "https://graphql.anilist.co";

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
        }
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
        let response = self
            .http
            .post(ENDPOINT)
            .header(
                "User-Agent",
                "Kaminari/0.1.0 (+https://github.com/graynb25/kaminari)",
            )
            .json(&serde_json::json!({ "query": query, "variables": variables }))
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
        let json = self
            .post(
                &trending_query(),
                serde_json::json!({ "page": page, "perPage": 24 }),
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

fn trending_query() -> String {
    media_query(
        r#"
query Trending($page: Int, $perPage: Int) {
  Page(page: $page, perPage: $perPage) {
    media(type: ANIME, sort: TRENDING_DESC) {
      __MEDIA_FIELDS__
    }
  }
}
"#,
    )
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
    fn surfaces_graphql_errors() {
        let raw = r#"{ "data": null, "errors": [{ "message": "Invalid query" }] }"#;
        let envelope: PageEnvelope = serde_json::from_str(raw).unwrap();
        let message = graphql_message(envelope.errors.unwrap());
        assert_eq!(message, "Invalid query");
    }
}
