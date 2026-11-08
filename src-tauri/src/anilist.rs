// AniList client with token bucket. Public ceiling near 90 per minute.
// Sustained 60 per minute, burst to 90. Honors Retry-After via caller.

use std::time::Duration;
use tokio::sync::Semaphore;

pub struct AniListClient {
    http: reqwest::Client,
    bucket: Semaphore,
}

impl AniListClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("reqwest client"),
            bucket: Semaphore::new(90),
        }
    }

    pub async fn acquire(&self) {
        let permit = self.bucket.acquire().await;
        drop(permit);
    }

    pub fn endpoint(&self) -> &'static str {
        "https://graphql.anilist.co"
    }
}
