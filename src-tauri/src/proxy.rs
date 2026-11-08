// Loopback stream proxy for protected MP4 and HLS playback.
// See docs/PLAN-STREAM-PROXY.md. The WebView gets an opaque loopback URL.
// Rust keeps the upstream URL, the allowlisted headers, the per entry token
// and the rewritten playlist children in memory only.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{StreamExt, TryStreamExt};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio_util::io::{ReaderStream, StreamReader};

use crate::ext_host::HostRule;

pub const PORTS: [u16; 8] = [39321, 39322, 39323, 39324, 39325, 39326, 39327, 39328];
pub const IDLE_TTL: Duration = Duration::from_secs(2 * 60 * 60);
pub const ABSOLUTE_TTL: Duration = Duration::from_secs(4 * 60 * 60);
pub const MAX_ENTRIES: usize = 64;
pub const MAX_HEADERS: usize = 16;
pub const MAX_HEADER_BYTES: usize = 8 * 1024;
pub const MAX_CHILDREN: usize = 256;
pub const MAX_PLAYLIST_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_KEY_BYTES: usize = 64 * 1024;
pub const MAX_SUBTITLE_BYTES: usize = 512 * 1024;
pub const MAX_RANGE_SKIP: u64 = 16 * 1024 * 1024;
pub const STREAM_CHUNK_BYTES: usize = 64 * 1024;
pub const SWEEP_SECS: u64 = 60;

const MAX_REDIRECTS: usize = 3;
const MAX_PLAYLIST_LINES: usize = 2000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(15);
const PLAYLIST_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const DNS_TIMEOUT: Duration = Duration::from_secs(10);
const EXPIRES_IN_SEC: u64 = 2 * 60 * 60;
const PLAYLIST_CONTENT_TYPE: &str = "application/vnd.apple.mpegurl";

const ALLOWED_HEADERS: [&str; 7] = [
    "user-agent",
    "referer",
    "origin",
    "cookie",
    "authorization",
    "accept",
    "accept-language",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyUrl {
    pub proxy_url: String,
    pub opaque_id: String,
    pub expires_in_sec: u64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub port: u16,
    pub entries: usize,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStreamInput {
    pub url: String,
    pub kind: String,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildKind {
    Playlist,
    Subtitle,
    Key,
    Segment,
    Map,
    File,
}

impl ChildKind {
    fn default_content_type(self) -> Option<&'static str> {
        match self {
            Self::Playlist | Self::Subtitle => Some(PLAYLIST_CONTENT_TYPE),
            Self::Key => Some("application/octet-stream"),
            Self::Segment | Self::Map | Self::File => None,
        }
    }
}

#[derive(Debug, Clone)]
struct ChildEntry {
    child_id: String,
    upstream_url: String,
    content_type: Option<String>,
    byte_range: Option<String>,
    kind: ChildKind,
    last_used: Instant,
}

#[derive(Debug, Clone)]
struct ChildSpec {
    upstream_url: String,
    content_type: Option<String>,
    byte_range: Option<String>,
    kind: ChildKind,
}

#[derive(Clone)]
struct ProxyEntry {
    opaque_id: String,
    token: String,
    extension_id: String,
    upstream_url: String,
    kind: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    allowed: Option<Vec<HostRule>>,
    allow_private: bool,
    created_at: Instant,
    last_used: Instant,
    children: HashMap<String, ChildEntry>,
}

struct StoreState {
    entries: HashMap<String, ProxyEntry>,
    port: u16,
    enabled: bool,
    allow_private: bool,
}

#[derive(Clone)]
pub struct ProxyStore {
    inner: Arc<Mutex<StoreState>>,
}

enum LookupError {
    NotFound,
    Forbidden,
    Expired,
}

#[derive(Debug)]
enum ProxyError {
    Upstream(&'static str),
    Timeout,
    MasterPlaylist,
    Oversized(&'static str),
    BadPlaylist,
}

struct ByteRange {
    start: u64,
    end: Option<u64>,
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

enum PlaylistLine {
    Text(String),
    UriTag {
        text: String,
        uri_start: usize,
        uri_end: usize,
        spec: usize,
    },
    Segment {
        spec: usize,
    },
}

struct ParsedPlaylist {
    lines: Vec<PlaylistLine>,
    specs: Vec<ChildSpec>,
}

impl ProxyStore {
    pub fn bind() -> (Self, Option<(std::net::TcpListener, Router)>) {
        let mut bound = None;
        let mut busy = 0usize;
        for port in PORTS {
            match std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
                Ok(listener) => {
                    if listener.set_nonblocking(true).is_err() {
                        continue;
                    }
                    bound = Some((port, listener));
                    break;
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::AddrInUse {
                        busy += 1;
                    }
                    tracing::debug!(port, error = %err, "proxy: port is not available");
                }
            }
        }

        match bound {
            Some((port, listener)) => {
                let store = Self::with_state(port, true, false);
                let router = store.router();
                tracing::info!(port, "proxy: listening on loopback");
                (store, Some((listener, router)))
            }
            None => {
                tracing::warn!(busy, "proxy: no loopback port was available");
                (Self::with_state(0, false, false), None)
            }
        }
    }

    fn with_state(port: u16, enabled: bool, allow_private: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(StoreState {
                entries: HashMap::new(),
                port,
                enabled,
                allow_private,
            })),
        }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/healthz", get(healthz).options(healthz_options))
            .route(
                "/s/{id}/file",
                get(file_handler).head(file_handler).options(route_options),
            )
            .route(
                "/s/{id}/manifest.m3u8",
                get(manifest_handler)
                    .head(manifest_handler)
                    .options(route_options),
            )
            .route(
                "/s/{id}/seg/{child}",
                get(segment_handler)
                    .head(segment_handler)
                    .options(route_options),
            )
            .layer(middleware::from_fn(cors))
            .with_state(self.clone())
    }

    pub fn status(&self) -> ProxyStatus {
        let state = self.inner.lock().expect("proxy store poisoned");
        ProxyStatus {
            port: state.port,
            entries: state.entries.len(),
            enabled: state.enabled,
        }
    }

    pub fn disable(&self) {
        if let Ok(mut state) = self.inner.lock() {
            state.enabled = false;
        }
    }

    pub async fn register(
        &self,
        extension_id: &str,
        stream: &ProxyStreamInput,
        allowed: Vec<HostRule>,
    ) -> Result<ProxyUrl, String> {
        self.register_inner(extension_id, stream, Some(allowed), false)
            .await
    }

    async fn register_inner(
        &self,
        extension_id: &str,
        stream: &ProxyStreamInput,
        allowed: Option<Vec<HostRule>>,
        allow_private: bool,
    ) -> Result<ProxyUrl, String> {
        let (enabled, store_allows_private) = {
            let state = self
                .inner
                .lock()
                .map_err(|_| "proxy store is unavailable")?;
            (state.enabled, state.allow_private)
        };
        if !enabled {
            return Err("proxy is not available".to_string());
        }

        let url = Url::parse(&stream.url).map_err(|_| "stream URL is invalid".to_string())?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err("stream URL must be http or https".to_string());
        }
        if url.host_str().is_none() {
            return Err("stream URL has no host".to_string());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("stream URL must not contain credentials".to_string());
        }
        if let Some(rules) = &allowed {
            if !rules.iter().any(|rule| rule.allows(&url)) {
                return Err("stream URL is not in the extension host allowlist".to_string());
            }
        }

        let headers = sanitize_headers(stream.headers.as_ref())?;
        let allow_private = allow_private || store_allows_private;
        resolve_checked(&url, allow_private).await?;

        let opaque_id = random_hex()?;
        let token = random_hex()?;
        let now = Instant::now();
        let kind = stream.kind.to_ascii_lowercase();
        let entry = ProxyEntry {
            opaque_id: opaque_id.clone(),
            token: token.clone(),
            extension_id: extension_id.to_string(),
            upstream_url: url.to_string(),
            kind: kind.clone(),
            headers,
            allowed,
            allow_private,
            created_at: now,
            last_used: now,
            children: HashMap::new(),
        };

        let port = {
            let mut state = self
                .inner
                .lock()
                .map_err(|_| "proxy store is unavailable")?;
            if !state.enabled {
                return Err("proxy is not available".to_string());
            }
            if state.entries.len() >= MAX_ENTRIES {
                if let Some(key) = state
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(key, _)| key.clone())
                {
                    if let Some(old) = state.entries.remove(&key) {
                        tracing::info!(
                            id = %id_prefix(&old.opaque_id),
                            extension = %old.extension_id,
                            "proxy: evicted the oldest entry"
                        );
                    }
                }
            }
            state.entries.insert(opaque_id.clone(), entry);
            state.port
        };

        tracing::info!(
            id = %id_prefix(&opaque_id),
            extension = %extension_id,
            kind = %stream.kind,
            "proxy: registered stream"
        );

        let route = if kind == "hls" {
            "manifest.m3u8"
        } else {
            "file"
        };
        Ok(ProxyUrl {
            proxy_url: format!("http://127.0.0.1:{port}/s/{opaque_id}/{route}?token={token}"),
            opaque_id,
            expires_in_sec: EXPIRES_IN_SEC,
            kind,
        })
    }

    pub fn revoke(&self, opaque_id: &str) -> Result<(), String> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "proxy store is unavailable")?;
        if let Some(entry) = state.entries.remove(opaque_id) {
            tracing::info!(
                id = %id_prefix(&entry.opaque_id),
                extension = %entry.extension_id,
                "proxy: revoked stream"
            );
        }
        Ok(())
    }

    fn lookup(&self, id: &str, token: &str) -> Result<ProxyEntry, LookupError> {
        let mut state = self.inner.lock().map_err(|_| LookupError::NotFound)?;
        let Some(entry) = state.entries.get(id) else {
            return Err(LookupError::NotFound);
        };
        check_entry(entry, token)?;
        let now = Instant::now();
        let entry = state.entries.get_mut(id).ok_or(LookupError::NotFound)?;
        entry.last_used = now;
        Ok(entry.clone())
    }

    fn lookup_child(
        &self,
        id: &str,
        token: &str,
        child_id: &str,
    ) -> Result<(ProxyEntry, ChildEntry), LookupError> {
        let mut state = self.inner.lock().map_err(|_| LookupError::NotFound)?;
        let Some(entry) = state.entries.get(id) else {
            return Err(LookupError::NotFound);
        };
        check_entry(entry, token)?;
        let now = Instant::now();
        let entry = state.entries.get_mut(id).ok_or(LookupError::NotFound)?;
        entry.last_used = now;
        let child = match entry.children.get_mut(child_id) {
            Some(child) => {
                child.last_used = now;
                child.clone()
            }
            None => return Err(LookupError::NotFound),
        };
        Ok((entry.clone(), child))
    }

    fn reconcile_children(&self, id: &str, specs: &[ChildSpec]) -> Result<Vec<String>, ProxyError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| ProxyError::Upstream("store"))?;
        let Some(entry) = state.entries.get_mut(id) else {
            return Err(ProxyError::Upstream("entry"));
        };
        let now = Instant::now();
        let mut ids = Vec::with_capacity(specs.len());
        for spec in specs {
            if let Some(child) = entry
                .children
                .values_mut()
                .find(|child| child.upstream_url == spec.upstream_url)
            {
                if spec.content_type.is_some() {
                    child.content_type = spec.content_type.clone();
                }
                child.byte_range = spec.byte_range.clone();
                child.kind = spec.kind;
                child.last_used = now;
                ids.push(child.child_id.clone());
                continue;
            }

            if entry.children.len() >= MAX_CHILDREN {
                if let Some(key) = entry
                    .children
                    .iter()
                    .min_by_key(|(_, child)| child.last_used)
                    .map(|(key, _)| key.clone())
                {
                    entry.children.remove(&key);
                }
            }

            let mut child_id = random_hex_n(8).map_err(|_| ProxyError::Upstream("random"))?;
            while entry.children.contains_key(&child_id) {
                child_id = random_hex_n(8).map_err(|_| ProxyError::Upstream("random"))?;
            }
            entry.children.insert(
                child_id.clone(),
                ChildEntry {
                    child_id: child_id.clone(),
                    upstream_url: spec.upstream_url.clone(),
                    content_type: spec.content_type.clone(),
                    byte_range: spec.byte_range.clone(),
                    kind: spec.kind,
                    last_used: now,
                },
            );
            ids.push(child_id);
        }
        Ok(ids)
    }

    pub fn sweep(&self) {
        let now = Instant::now();
        let mut removed = 0usize;
        let mut state = match self.inner.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        state.entries.retain(|_, entry| {
            let expired = now.duration_since(entry.created_at) >= ABSOLUTE_TTL
                || now.duration_since(entry.last_used) >= IDLE_TTL;
            if expired {
                removed += 1;
                tracing::info!(
                    id = %id_prefix(&entry.opaque_id),
                    extension = %entry.extension_id,
                    "proxy: swept an expired entry"
                );
            }
            !expired
        });
        if removed > 0 {
            tracing::info!(removed, "proxy: sweep finished");
        }
    }

    pub async fn sweep_loop(&self) {
        let mut ticker = tokio::time::interval(Duration::from_secs(SWEEP_SECS));
        loop {
            ticker.tick().await;
            self.sweep();
        }
    }

    #[cfg(test)]
    fn for_tests(port: u16) -> Self {
        Self::with_state(port, true, true)
    }

    #[cfg(test)]
    async fn register_fixture(
        &self,
        extension_id: &str,
        url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<ProxyUrl, String> {
        self.register_fixture_with_kind(extension_id, url, "mp4", headers)
            .await
    }

    #[cfg(test)]
    async fn register_fixture_with_kind(
        &self,
        extension_id: &str,
        url: &str,
        kind: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<ProxyUrl, String> {
        let stream = ProxyStreamInput {
            url: url.to_string(),
            kind: kind.to_string(),
            headers,
        };
        self.register_inner(extension_id, &stream, None, true).await
    }

    #[cfg(test)]
    fn expire_for_test(&self, opaque_id: &str) {
        let mut state = self.inner.lock().unwrap();
        if let Some(entry) = state.entries.get_mut(opaque_id) {
            entry.last_used = Instant::now() - IDLE_TTL - Duration::from_secs(1);
        }
    }
}

async fn cors(request: Request, next: Next) -> Response {
    let options = request.method() == Method::OPTIONS;
    let mut response = next.run(request).await;
    let status = response.status();
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Range"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Content-Range, Content-Length, Accept-Ranges"),
    );
    if options && status == StatusCode::NO_CONTENT {
        headers.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("600"),
        );
    }
    response
}

async fn healthz(State(store): State<ProxyStore>) -> Response {
    let status = store.status();
    axum::Json(serde_json::json!({
        "ok": status.enabled,
        "port": status.port,
    }))
    .into_response()
}

async fn healthz_options() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn route_options(
    State(store): State<ProxyStore>,
    Path(params): Path<HashMap<String, String>>,
    Query(query): Query<TokenQuery>,
) -> Response {
    let id = params.get("id").cloned().unwrap_or_default();
    let token = query.token.unwrap_or_default();
    match store.lookup(&id, &token) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(LookupError::NotFound) => short(StatusCode::NOT_FOUND, "not-found"),
        Err(LookupError::Forbidden) => {
            tracing::warn!(id = %id_prefix(&id), "proxy: rejected a bad token");
            short(StatusCode::FORBIDDEN, "forbidden")
        }
        Err(LookupError::Expired) => short(StatusCode::GONE, "expired"),
    }
}

async fn manifest_handler(
    State(store): State<ProxyStore>,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
    method: Method,
) -> Response {
    let token = query.token.unwrap_or_default();
    let entry = match store.lookup(&id, &token) {
        Ok(entry) => entry,
        Err(LookupError::NotFound) => return short(StatusCode::NOT_FOUND, "not-found"),
        Err(LookupError::Forbidden) => {
            tracing::warn!(id = %id_prefix(&id), "proxy: rejected a bad token");
            return short(StatusCode::FORBIDDEN, "forbidden");
        }
        Err(LookupError::Expired) => return short(StatusCode::GONE, "expired"),
    };
    if entry.kind != "hls" {
        return short(StatusCode::NOT_FOUND, "not-hls");
    }

    let (upstream, final_url) =
        match fetch_upstream_from(&entry, &entry.upstream_url, &Method::GET, None).await {
            Ok(fetched) => fetched,
            Err(err) => return proxy_error_response(err),
        };
    if !upstream.status().is_success() {
        tracing::debug!(
            id = %id_prefix(&entry.opaque_id),
            status = upstream.status().as_u16(),
            "proxy: playlist upstream refused"
        );
        return short(StatusCode::BAD_GATEWAY, "upstream-error");
    }
    let content_type = header_string(upstream.headers(), header::CONTENT_TYPE);
    let body = match read_capped(upstream, MAX_PLAYLIST_BYTES, "playlist-too-large").await {
        Ok(body) => body,
        Err(err) => return proxy_error_response(err),
    };
    if !is_playlist_content_type(content_type.as_deref()) && !looks_like_playlist(&body) {
        return proxy_error_response(ProxyError::BadPlaylist);
    }
    let text = String::from_utf8_lossy(&body);
    if is_master_playlist(&text) {
        return proxy_error_response(ProxyError::MasterPlaylist);
    }
    let parsed = match parse_media_playlist(&final_url, &text) {
        Ok(parsed) => parsed,
        Err(err) => return proxy_error_response(err),
    };
    let child_ids = match store.reconcile_children(&id, &parsed.specs) {
        Ok(ids) => ids,
        Err(err) => return proxy_error_response(err),
    };
    let rewritten = render_playlist(&entry, &parsed, &child_ids);
    playlist_response(rewritten, &method)
}

async fn segment_handler(
    State(store): State<ProxyStore>,
    Path((id, child_id)): Path<(String, String)>,
    Query(query): Query<TokenQuery>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let token = query.token.unwrap_or_default();
    let (entry, child) = match store.lookup_child(&id, &token, &child_id) {
        Ok(found) => found,
        Err(LookupError::NotFound) => return short(StatusCode::NOT_FOUND, "not-found"),
        Err(LookupError::Forbidden) => {
            tracing::warn!(id = %id_prefix(&id), "proxy: rejected a bad token");
            return short(StatusCode::FORBIDDEN, "forbidden");
        }
        Err(LookupError::Expired) => return short(StatusCode::GONE, "expired"),
    };
    if entry.kind != "hls" {
        return short(StatusCode::NOT_FOUND, "not-hls");
    }

    match child.kind {
        ChildKind::Playlist | ChildKind::Subtitle => {
            playlist_child_response(&store, &entry, &child, &method).await
        }
        ChildKind::Key => {
            buffered_child_response(&entry, &child, &method, MAX_KEY_BYTES, "key-too-large").await
        }
        ChildKind::File => {
            buffered_child_response(
                &entry,
                &child,
                &method,
                MAX_SUBTITLE_BYTES,
                "subtitle-too-large",
            )
            .await
        }
        ChildKind::Segment | ChildKind::Map => {
            stream_child_response(&entry, &child, &method, &headers).await
        }
    }
}

async fn playlist_child_response(
    store: &ProxyStore,
    entry: &ProxyEntry,
    child: &ChildEntry,
    method: &Method,
) -> Response {
    let cap = if child.kind == ChildKind::Subtitle {
        MAX_SUBTITLE_BYTES
    } else {
        MAX_PLAYLIST_BYTES
    };
    let too_large = if child.kind == ChildKind::Subtitle {
        "subtitle-too-large"
    } else {
        "playlist-too-large"
    };
    let (upstream, final_url) =
        match fetch_upstream_from(entry, &child.upstream_url, &Method::GET, None).await {
            Ok(fetched) => fetched,
            Err(err) => return proxy_error_response(err),
        };
    if !upstream.status().is_success() {
        return short(StatusCode::BAD_GATEWAY, "upstream-error");
    }
    let content_type = header_string(upstream.headers(), header::CONTENT_TYPE);
    let body = match read_capped(upstream, cap, too_large).await {
        Ok(body) => body,
        Err(err) => return proxy_error_response(err),
    };
    let is_playlist =
        looks_like_playlist(&body) || is_playlist_content_type(content_type.as_deref());
    if !is_playlist && child.kind == ChildKind::Subtitle {
        return buffered_response(method, body, content_type.as_deref(), "text/vtt", true);
    }
    if !is_playlist {
        return proxy_error_response(ProxyError::BadPlaylist);
    }
    let text = String::from_utf8_lossy(&body);
    if is_master_playlist(&text) {
        return proxy_error_response(ProxyError::MasterPlaylist);
    }
    let parsed = match parse_media_playlist(&final_url, &text) {
        Ok(parsed) => parsed,
        Err(err) => return proxy_error_response(err),
    };
    let child_ids = match store.reconcile_children(&entry.opaque_id, &parsed.specs) {
        Ok(ids) => ids,
        Err(err) => return proxy_error_response(err),
    };
    let rewritten = render_playlist(entry, &parsed, &child_ids);
    playlist_response(rewritten, method)
}

async fn buffered_child_response(
    entry: &ProxyEntry,
    child: &ChildEntry,
    method: &Method,
    cap: usize,
    too_large: &'static str,
) -> Response {
    let (upstream, _) =
        match fetch_upstream_from(entry, &child.upstream_url, &Method::GET, None).await {
            Ok(fetched) => fetched,
            Err(err) => return proxy_error_response(err),
        };
    if !upstream.status().is_success() {
        return short(StatusCode::BAD_GATEWAY, "upstream-error");
    }
    let content_type = header_string(upstream.headers(), header::CONTENT_TYPE);
    let body = match read_capped(upstream, cap, too_large).await {
        Ok(body) => body,
        Err(err) => return proxy_error_response(err),
    };
    let fallback = child
        .content_type
        .as_deref()
        .or_else(|| child.kind.default_content_type())
        .unwrap_or("application/octet-stream");
    buffered_response(method, body, content_type.as_deref(), fallback, true)
}

async fn stream_child_response(
    entry: &ProxyEntry,
    child: &ChildEntry,
    method: &Method,
    headers: &HeaderMap,
) -> Response {
    let requested = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let effective = requested.clone().or_else(|| {
        child
            .byte_range
            .as_ref()
            .map(|range| format!("bytes={range}"))
    });
    let range = effective.as_deref().and_then(parse_range);

    let upstream =
        match fetch_upstream_from(entry, &child.upstream_url, method, effective.as_deref()).await {
            Ok((response, _)) => response,
            Err(err) => return proxy_error_response(err),
        };

    stream_response(upstream, method, range, effective.is_some()).await
}

fn playlist_response(body: String, method: &Method) -> Response {
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(body)
    };
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(PLAYLIST_CONTENT_TYPE),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("none"));
    response
}

fn buffered_response(
    method: &Method,
    body: Vec<u8>,
    upstream_type: Option<&str>,
    fallback_type: &str,
    no_store: bool,
) -> Response {
    let content_type = upstream_type.unwrap_or(fallback_type);
    let content_type = HeaderValue::from_str(content_type)
        .unwrap_or(HeaderValue::from_static("application/octet-stream"));
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(body)
    };
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, content_type);
    if no_store {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

async fn file_handler(
    State(store): State<ProxyStore>,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let token = query.token.unwrap_or_default();
    let entry = match store.lookup(&id, &token) {
        Ok(entry) => entry,
        Err(LookupError::NotFound) => return short(StatusCode::NOT_FOUND, "not-found"),
        Err(LookupError::Forbidden) => {
            tracing::warn!(id = %id_prefix(&id), "proxy: rejected a bad token");
            return short(StatusCode::FORBIDDEN, "forbidden");
        }
        Err(LookupError::Expired) => return short(StatusCode::GONE, "expired"),
    };

    let range_header = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let range = range_header.as_deref().and_then(parse_range);

    tracing::debug!(
        id = %id_prefix(&entry.opaque_id),
        kind = %entry.kind,
        method = %method,
        range = range_header.is_some(),
        "proxy: serving stream"
    );

    let upstream = match fetch_upstream(&entry, &method, range_header.as_deref()).await {
        Ok(response) => response,
        Err(err) => return proxy_error_response(err),
    };

    stream_response(upstream, &method, range, range_header.is_some()).await
}

async fn fetch_upstream(
    entry: &ProxyEntry,
    method: &Method,
    range: Option<&str>,
) -> Result<reqwest::Response, ProxyError> {
    fetch_upstream_from(entry, &entry.upstream_url, method, range)
        .await
        .map(|(response, _)| response)
}

async fn fetch_upstream_from(
    entry: &ProxyEntry,
    start_url: &str,
    method: &Method,
    range: Option<&str>,
) -> Result<(reqwest::Response, Url), ProxyError> {
    let original = Url::parse(start_url).map_err(|_| ProxyError::Upstream("url"))?;
    let mut current = original.clone();

    for hop in 0..=MAX_REDIRECTS {
        if let Some(rules) = &entry.allowed {
            if !rules.iter().any(|rule| rule.allows(&current)) {
                return Err(ProxyError::Upstream("allowlist"));
            }
        }
        let addresses = resolve_checked(&current, entry.allow_private)
            .await
            .map_err(|_| ProxyError::Upstream("resolve"))?;
        let host = current
            .host_str()
            .ok_or(ProxyError::Upstream("host"))?
            .to_string();

        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(IDLE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_gzip()
            .resolve_to_addrs(&host, &addresses)
            .build()
            .map_err(|_| ProxyError::Upstream("client"))?;

        let mut request = client.request(method.clone(), current.clone());
        let same_origin = current.origin() == original.origin();
        for (name, value) in &entry.headers {
            if !same_origin && (*name == header::AUTHORIZATION || *name == header::COOKIE) {
                continue;
            }
            request = request.header(name, value);
        }
        if let Some(value) = range {
            request = request.header(header::RANGE, value);
        }

        let response = tokio::time::timeout(FIRST_BYTE_TIMEOUT, request.send())
            .await
            .map_err(|_| ProxyError::Timeout)?
            .map_err(|_| ProxyError::Upstream("send"))?;

        if !response.status().is_redirection() {
            return Ok((response, current));
        }
        if hop == MAX_REDIRECTS {
            return Err(ProxyError::Upstream("redirects"));
        }
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ProxyError::Upstream("location"))?;
        let next = current
            .join(location)
            .map_err(|_| ProxyError::Upstream("location"))?;
        if next.scheme() != "http" && next.scheme() != "https" {
            return Err(ProxyError::Upstream("scheme"));
        }
        tracing::debug!(
            id = %id_prefix(&entry.opaque_id),
            host = %next.host_str().unwrap_or(""),
            "proxy: following redirect"
        );
        current = next;
    }

    Err(ProxyError::Upstream("redirects"))
}

async fn stream_response(
    upstream: reqwest::Response,
    method: &Method,
    range: Option<ByteRange>,
    range_requested: bool,
) -> Response {
    let status = upstream.status();
    if status == StatusCode::RANGE_NOT_SATISFIABLE {
        return relay(upstream, StatusCode::RANGE_NOT_SATISFIABLE, method);
    }
    if !status.is_success() {
        tracing::debug!(
            status = status.as_u16(),
            "proxy: upstream refused the request"
        );
        return short(StatusCode::BAD_GATEWAY, "upstream-error");
    }
    if method == Method::HEAD {
        return relay(upstream, status, method);
    }
    if status == StatusCode::PARTIAL_CONTENT {
        return relay(upstream, StatusCode::PARTIAL_CONTENT, method);
    }
    if range_requested {
        let Some(range) = range else {
            return short(StatusCode::BAD_GATEWAY, "range-unsupported");
        };
        return skip_to_range(upstream, range).await;
    }
    relay(upstream, status, method)
}

async fn skip_to_range(upstream: reqwest::Response, range: ByteRange) -> Response {
    if range.start > MAX_RANGE_SKIP {
        return short(StatusCode::BAD_GATEWAY, "range-unsupported");
    }
    let total = upstream.content_length();
    if let Some(total) = total {
        if range.start >= total {
            return short(StatusCode::RANGE_NOT_SATISFIABLE, "range-not-satisfiable");
        }
    }

    let mut reader = StreamReader::new(upstream.bytes_stream().map_err(std::io::Error::other));
    let mut remaining = range.start;
    let mut buf = vec![0u8; STREAM_CHUNK_BYTES];
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let read = match reader.read(&mut buf[..want]).await {
            Ok(read) => read,
            Err(_) => return proxy_error_response(ProxyError::Upstream("read")),
        };
        if read == 0 {
            return short(StatusCode::RANGE_NOT_SATISFIABLE, "range-not-satisfiable");
        }
        remaining -= read as u64;
    }

    let last = match (range.end, total) {
        (Some(end), Some(total)) => Some(end.min(total.saturating_sub(1))),
        (Some(end), None) => Some(end),
        (None, Some(total)) => Some(total.saturating_sub(1)),
        (None, None) => None,
    };
    let content_range = match (last, total) {
        (Some(end), Some(total)) => format!("bytes {}-{}/{}", range.start, end, total),
        (Some(end), None) => format!("bytes {}-{}/*", range.start, end),
        (None, Some(total)) => format!(
            "bytes {}-{}/{}",
            range.start,
            total.saturating_sub(1),
            total
        ),
        (None, None) => return short(StatusCode::BAD_GATEWAY, "range-unsupported"),
    };

    let mut response = Response::new(Body::from_stream(ReaderStream::with_capacity(
        reader,
        STREAM_CHUNK_BYTES,
    )));
    *response.status_mut() = StatusCode::PARTIAL_CONTENT;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_RANGE,
        HeaderValue::from_str(&content_range).unwrap_or(HeaderValue::from_static("bytes */*")),
    );
    if let Some(end) = last {
        if let Ok(value) = HeaderValue::from_str(&(end - range.start + 1).to_string()) {
            headers.insert(header::CONTENT_LENGTH, value);
        }
    }
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response
}

fn relay(upstream: reqwest::Response, status: StatusCode, method: &Method) -> Response {
    let mut headers = HeaderMap::new();
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
        header::CONTENT_ENCODING,
        header::LAST_MODIFIED,
        header::ETAG,
    ] {
        if let Some(value) = upstream.headers().get(&name) {
            headers.insert(name, value.clone());
        }
    }
    if !headers.contains_key(header::ACCEPT_RANGES) {
        headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    }

    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(ReaderStream::with_capacity(
            StreamReader::new(upstream.bytes_stream().map_err(std::io::Error::other)),
            STREAM_CHUNK_BYTES,
        ))
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    response
}

fn proxy_error_response(error: ProxyError) -> Response {
    match error {
        ProxyError::Timeout => short(StatusCode::GATEWAY_TIMEOUT, "timeout"),
        ProxyError::MasterPlaylist => {
            short(StatusCode::NOT_IMPLEMENTED, "master-playlist-unsupported")
        }
        ProxyError::Oversized(reason) => (StatusCode::BAD_GATEWAY, reason).into_response(),
        ProxyError::BadPlaylist => short(StatusCode::BAD_GATEWAY, "not-a-playlist"),
        ProxyError::Upstream(reason) => {
            tracing::debug!(reason, "proxy: upstream request failed");
            short(StatusCode::BAD_GATEWAY, "upstream-error")
        }
    }
}

fn short(status: StatusCode, body: &'static str) -> Response {
    (status, body).into_response()
}

fn check_entry(entry: &ProxyEntry, token: &str) -> Result<(), LookupError> {
    if !constant_time_eq(entry.token.as_bytes(), token.as_bytes()) {
        return Err(LookupError::Forbidden);
    }
    let now = Instant::now();
    if now.duration_since(entry.created_at) >= ABSOLUTE_TTL
        || now.duration_since(entry.last_used) >= IDLE_TTL
    {
        return Err(LookupError::Expired);
    }
    Ok(())
}

fn header_string(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

async fn read_capped(
    response: reqwest::Response,
    cap: usize,
    too_large: &'static str,
) -> Result<Vec<u8>, ProxyError> {
    if let Some(length) = response.content_length() {
        if length > cap as u64 {
            return Err(ProxyError::Oversized(too_large));
        }
    }
    let mut stream = response.bytes_stream();
    tokio::time::timeout(PLAYLIST_TIMEOUT, async move {
        let mut out = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| ProxyError::Upstream("read"))?;
            if out.len() + chunk.len() > cap {
                return Err(ProxyError::Oversized(too_large));
            }
            out.extend_from_slice(&chunk);
        }
        Ok(out)
    })
    .await
    .map_err(|_| ProxyError::Timeout)?
}

fn is_playlist_content_type(value: Option<&str>) -> bool {
    value
        .map(str::to_ascii_lowercase)
        .is_some_and(|value| value.contains("mpegurl"))
}

fn looks_like_playlist(body: &[u8]) -> bool {
    let text = String::from_utf8_lossy(body);
    text.trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with("#EXTM3U")
}

fn is_master_playlist(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("#EXT-X-STREAM-INF") || line.starts_with("#EXT-X-I-FRAME-STREAM-INF")
    })
}

fn parse_media_playlist(base: &Url, text: &str) -> Result<ParsedPlaylist, ProxyError> {
    let mut lines = Vec::new();
    let mut specs = Vec::new();
    let mut defines: HashMap<String, String> = HashMap::new();
    let mut pending_range: Option<String> = None;
    let mut next_offset = 0u64;
    let mut count = 0usize;

    for raw in text.lines() {
        count += 1;
        if count > MAX_PLAYLIST_LINES {
            return Err(ProxyError::Oversized("playlist-too-large"));
        }
        let line = raw.trim();
        if line.is_empty() {
            lines.push(PlaylistLine::Text(String::new()));
            continue;
        }
        if line.starts_with("#EXT-X-STREAM-INF") || line.starts_with("#EXT-X-I-FRAME-STREAM-INF") {
            return Err(ProxyError::MasterPlaylist);
        }
        if line.starts_with("#EXT-X-DEFINE") {
            if has_attr(line, "IMPORT") {
                tracing::debug!(tag = %tag_name(line), "proxy: dropped a playlist import");
                continue;
            }
            if let (Some(name), Some(value)) = (attr_value(line, "NAME"), attr_value(line, "VALUE"))
            {
                defines.insert(name.to_string(), value.to_string());
            }
            lines.push(PlaylistLine::Text(line.to_string()));
            continue;
        }
        if line.starts_with("#EXT-X-BYTERANGE") {
            if let Some((_, value)) = line.split_once(':') {
                pending_range = parse_byte_range(value, &mut next_offset);
            }
            lines.push(PlaylistLine::Text(line.to_string()));
            continue;
        }
        if line.starts_with("#EXT-X-KEY") || line.starts_with("#EXT-X-SESSION-KEY") {
            if attr_value(line, "METHOD").is_some_and(|method| method.eq_ignore_ascii_case("NONE"))
            {
                lines.push(PlaylistLine::Text(line.to_string()));
            } else {
                push_uri_tag(
                    line,
                    ChildKind::Key,
                    None,
                    &defines,
                    base,
                    &mut lines,
                    &mut specs,
                );
            }
            continue;
        }
        if line.starts_with("#EXT-X-MAP") {
            let mut map_offset = 0u64;
            let range = attr_value(line, "BYTERANGE")
                .and_then(|value| parse_byte_range(value, &mut map_offset));
            push_uri_tag(
                line,
                ChildKind::Map,
                range,
                &defines,
                base,
                &mut lines,
                &mut specs,
            );
            continue;
        }
        if line.starts_with("#EXT-X-MEDIA") {
            let kind = match attr_value(line, "TYPE") {
                Some(kind) if kind.eq_ignore_ascii_case("SUBTITLES") => ChildKind::Subtitle,
                _ => ChildKind::Playlist,
            };
            push_uri_tag(line, kind, None, &defines, base, &mut lines, &mut specs);
            continue;
        }
        if line.starts_with("#EXT-X-SESSION-DATA") {
            push_uri_tag(
                line,
                ChildKind::File,
                None,
                &defines,
                base,
                &mut lines,
                &mut specs,
            );
            continue;
        }
        if line.starts_with("#EXT-X-PART") {
            let mut part_offset = 0u64;
            let range = attr_value(line, "BYTERANGE")
                .and_then(|value| parse_byte_range(value, &mut part_offset));
            push_uri_tag(
                line,
                ChildKind::Segment,
                range,
                &defines,
                base,
                &mut lines,
                &mut specs,
            );
            continue;
        }
        if line.starts_with("#EXT-X-PRELOAD-HINT") {
            push_uri_tag(
                line,
                ChildKind::Segment,
                None,
                &defines,
                base,
                &mut lines,
                &mut specs,
            );
            continue;
        }
        if line.starts_with('#') {
            lines.push(PlaylistLine::Text(line.to_string()));
            continue;
        }

        let Some(uri) = substitute_defines(line, &defines) else {
            tracing::debug!(
                tag = "segment",
                "proxy: dropped a playlist URI with an undefined variable"
            );
            continue;
        };
        let Some(resolved) = resolve_playlist_url(base, &uri) else {
            tracing::debug!(tag = "segment", "proxy: dropped a malformed playlist URI");
            continue;
        };
        let spec = ChildSpec {
            upstream_url: resolved.to_string(),
            content_type: None,
            byte_range: pending_range.take(),
            kind: ChildKind::Segment,
        };
        let index = specs.len();
        specs.push(spec);
        lines.push(PlaylistLine::Segment { spec: index });
    }

    Ok(ParsedPlaylist { lines, specs })
}

#[derive(Clone, Copy)]
struct Attribute<'a> {
    name: &'a str,
    value: &'a str,
    value_start: usize,
    value_end: usize,
}

fn parse_attributes(line: &str) -> Vec<Attribute<'_>> {
    let start = line.find(':').map(|index| index + 1).unwrap_or(0);
    let bytes = line.as_bytes();
    let mut attrs = Vec::new();
    let mut index = start;

    while index < line.len() {
        while index < line.len() && (bytes[index] == b',' || bytes[index].is_ascii_whitespace()) {
            index += 1;
        }
        let name_start = index;
        while index < line.len() && bytes[index] != b'=' && bytes[index] != b',' {
            index += 1;
        }
        if index >= line.len() || bytes[index] != b'=' {
            break;
        }
        let name = line[name_start..index].trim();
        index += 1;
        if index < line.len() && bytes[index] == b'"' {
            index += 1;
            let value_start = index;
            while index < line.len() && bytes[index] != b'"' {
                index += 1;
            }
            let value_end = index;
            if index < line.len() {
                index += 1;
            }
            attrs.push(Attribute {
                name,
                value: &line[value_start..value_end],
                value_start,
                value_end,
            });
        } else {
            let value_start = index;
            while index < line.len() && bytes[index] != b',' {
                index += 1;
            }
            let value_end = index;
            attrs.push(Attribute {
                name,
                value: line[value_start..value_end].trim(),
                value_start,
                value_end,
            });
        }
    }

    attrs
}

fn find_attr<'a>(line: &'a str, key: &str) -> Option<Attribute<'a>> {
    parse_attributes(line)
        .into_iter()
        .find(|attr| attr.name.eq_ignore_ascii_case(key))
}

fn attr_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    find_attr(line, key).map(|attr| attr.value)
}

fn has_attr(line: &str, key: &str) -> bool {
    find_attr(line, key).is_some()
}

fn tag_name(line: &str) -> &str {
    line.split([':', ',']).next().unwrap_or("tag")
}

fn push_uri_tag(
    line: &str,
    kind: ChildKind,
    byte_range: Option<String>,
    defines: &HashMap<String, String>,
    base: &Url,
    lines: &mut Vec<PlaylistLine>,
    specs: &mut Vec<ChildSpec>,
) {
    let Some(attr) = find_attr(line, "URI") else {
        lines.push(PlaylistLine::Text(line.to_string()));
        return;
    };
    let Some(uri) = substitute_defines(attr.value, defines) else {
        tracing::debug!(tag = %tag_name(line), "proxy: dropped a playlist URI with an undefined variable");
        return;
    };
    let Some(resolved) = resolve_playlist_url(base, &uri) else {
        tracing::debug!(tag = %tag_name(line), "proxy: dropped a malformed playlist URI");
        return;
    };
    let spec = ChildSpec {
        upstream_url: resolved.to_string(),
        content_type: kind.default_content_type().map(str::to_string),
        byte_range,
        kind,
    };
    let index = specs.len();
    specs.push(spec);
    lines.push(PlaylistLine::UriTag {
        text: line.to_string(),
        uri_start: attr.value_start,
        uri_end: attr.value_end,
        spec: index,
    });
}

fn substitute_defines(value: &str, defines: &HashMap<String, String>) -> Option<String> {
    if !value.contains("{$") {
        return Some(value.to_string());
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("{$") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}')?;
        let name = &after[..end];
        out.push_str(defines.get(name)?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

fn resolve_playlist_url(base: &Url, raw: &str) -> Option<Url> {
    let url = base.join(raw.trim()).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    url.host_str()?;
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    Some(url)
}

fn parse_byte_range(value: &str, next_offset: &mut u64) -> Option<String> {
    let value = value.trim();
    let (length, offset) = match value.split_once('@') {
        Some((length, offset)) => (length, Some(offset)),
        None => (value, None),
    };
    let length: u64 = length.trim().parse().ok()?;
    if length == 0 {
        return None;
    }
    let start = match offset {
        Some(offset) => offset.trim().parse().ok()?,
        None => *next_offset,
    };
    let end = start.checked_add(length)?.saturating_sub(1);
    *next_offset = end.saturating_add(1);
    Some(format!("{start}-{end}"))
}

fn render_playlist(entry: &ProxyEntry, parsed: &ParsedPlaylist, child_ids: &[String]) -> String {
    let mut out = String::new();
    for line in &parsed.lines {
        match line {
            PlaylistLine::Text(text) => {
                out.push_str(text);
                out.push('\n');
            }
            PlaylistLine::UriTag {
                text,
                uri_start,
                uri_end,
                spec,
            } => {
                if let Some(child_id) = child_ids.get(*spec) {
                    out.push_str(&text[..*uri_start]);
                    out.push_str(&child_path(entry, child_id));
                    out.push_str(&text[*uri_end..]);
                } else {
                    out.push_str(text);
                }
                out.push('\n');
            }
            PlaylistLine::Segment { spec } => {
                if let Some(child_id) = child_ids.get(*spec) {
                    out.push_str(&child_path(entry, child_id));
                }
                out.push('\n');
            }
        }
    }
    out
}

fn child_path(entry: &ProxyEntry, child_id: &str) -> String {
    format!(
        "/s/{}/seg/{}?token={}",
        entry.opaque_id, child_id, entry.token
    )
}

fn sanitize_headers(
    input: Option<&HashMap<String, String>>,
) -> Result<Vec<(HeaderName, HeaderValue)>, String> {
    let Some(input) = input else {
        return Ok(Vec::new());
    };
    if input.len() > MAX_HEADERS {
        return Err(format!("too many headers, the limit is {MAX_HEADERS}"));
    }

    let mut out = Vec::new();
    let mut bytes = 0usize;
    for (name, value) in input {
        let lower = name.trim().to_ascii_lowercase();
        if lower.is_empty() {
            continue;
        }
        if !ALLOWED_HEADERS.contains(&lower.as_str()) {
            tracing::debug!(header = %lower, "proxy: dropped a header");
            continue;
        }
        if lower.contains(['\r', '\n', '\0']) || value.contains(['\r', '\n', '\0']) {
            return Err("headers must not contain CR, LF or NUL".to_string());
        }
        let header_name = HeaderName::from_bytes(lower.as_bytes())
            .map_err(|_| "header name is invalid".to_string())?;
        let header_value =
            HeaderValue::from_str(value).map_err(|_| "header value is invalid".to_string())?;
        bytes += lower.len() + value.len();
        if bytes > MAX_HEADER_BYTES {
            return Err("headers are larger than 8 KB".to_string());
        }
        out.push((header_name, header_value));
    }
    Ok(out)
}

async fn resolve_checked(url: &Url, allow_private: bool) -> Result<Vec<SocketAddr>, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "URL has no port".to_string())?;

    let lower = host.trim_end_matches('.').to_ascii_lowercase();
    if !allow_private
        && (lower == "localhost"
            || lower.ends_with(".localhost")
            || lower.ends_with(".local")
            || lower.ends_with(".internal"))
    {
        return Err("upstream host is local".to_string());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        check_ip(ip, allow_private)?;
        return Ok(vec![SocketAddr::new(ip, port)]);
    }

    let lookup = tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((host, port)))
        .await
        .map_err(|_| "upstream DNS lookup timed out".to_string())?
        .map_err(|_| "upstream host could not be resolved".to_string())?;
    let addresses: Vec<SocketAddr> = lookup.collect();
    if addresses.is_empty() {
        return Err("upstream host has no addresses".to_string());
    }
    for address in &addresses {
        check_ip(address.ip(), allow_private)?;
    }
    Ok(addresses)
}

fn check_ip(ip: IpAddr, allow_private: bool) -> Result<(), String> {
    if allow_private {
        return Ok(());
    }
    if is_blocked_ip(ip) {
        return Err("upstream address is not public".to_string());
    }
    Ok(())
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || ip.octets() == [169, 254, 169, 254]
                || ip.octets()[0] == 0
        }
        IpAddr::V6(ip) => {
            ip.to_ipv4_mapped()
                .is_some_and(|ip| is_blocked_ip(IpAddr::V4(ip)))
                || ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
        }
    }
}

fn parse_range(value: &str) -> Option<ByteRange> {
    let rest = value.strip_prefix("bytes=")?;
    if rest.contains(',') {
        return None;
    }
    let (start, end) = rest.split_once('-')?;
    let start: u64 = start.parse().ok()?;
    let end = if end.is_empty() {
        None
    } else {
        Some(end.parse::<u64>().ok()?)
    };
    if end.is_some_and(|end| end < start) {
        return None;
    }
    Some(ByteRange { start, end })
}

fn random_hex() -> Result<String, String> {
    random_hex_n(16)
}

fn random_hex_n(len: usize) -> Result<String, String> {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes).map_err(|err| format!("random source failed: {err}"))?;
    Ok(hex::encode(bytes))
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

fn id_prefix(id: &str) -> &str {
    id.get(..6).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get as test_get;
    use axum::Router;
    use serde_json::Value;
    use std::sync::Mutex as StdMutex;
    use tokio::net::TcpListener as TestListener;

    struct Fixture {
        base: String,
        seen: Arc<StdMutex<Vec<HeaderMap>>>,
        body: Arc<Vec<u8>>,
        _handle: tokio::task::JoinHandle<()>,
    }

    impl Fixture {
        fn last_headers(&self) -> HeaderMap {
            self.seen
                .lock()
                .unwrap()
                .last()
                .cloned()
                .unwrap_or_default()
        }
    }

    async fn fixture() -> Fixture {
        let body = Arc::new(
            (0..2 * 1024 * 1024)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<u8>>(),
        );
        let seen = Arc::new(StdMutex::new(Vec::new()));

        let range_body = body.clone();
        let range_seen = seen.clone();
        let range = move |headers: HeaderMap| async move {
            range_seen.lock().unwrap().push(headers.clone());
            let full = range_body.as_slice();
            match headers
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_range)
            {
                Some(range) => {
                    let end = range
                        .end
                        .unwrap_or((full.len() as u64).saturating_sub(1))
                        .min((full.len() as u64).saturating_sub(1));
                    let start = range.start.min(full.len() as u64);
                    let slice = &full[start as usize..=end as usize];
                    let mut out = HeaderMap::new();
                    out.insert(
                        header::CONTENT_RANGE,
                        HeaderValue::from_str(&format!("bytes {start}-{end}/{}", full.len()))
                            .unwrap(),
                    );
                    out.insert(
                        header::CONTENT_LENGTH,
                        HeaderValue::from_str(&slice.len().to_string()).unwrap(),
                    );
                    out.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
                    (StatusCode::PARTIAL_CONTENT, out, slice.to_vec())
                }
                None => {
                    let mut out = HeaderMap::new();
                    out.insert(
                        header::CONTENT_LENGTH,
                        HeaderValue::from_str(&full.len().to_string()).unwrap(),
                    );
                    out.insert(header::ACCEPT_RANGES, HeaderValue::from_static("none"));
                    (StatusCode::OK, out, full.to_vec())
                }
            }
        };

        let ignore_body = body.clone();
        let ignore_seen = seen.clone();
        let ignores_range = move |headers: HeaderMap| async move {
            ignore_seen.lock().unwrap().push(headers);
            let mut out = HeaderMap::new();
            out.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&ignore_body.len().to_string()).unwrap(),
            );
            (StatusCode::OK, out, ignore_body.to_vec())
        };

        let small_body = Arc::new(vec![7u8; 1024]);
        let small = { small_body.clone() };
        let small_range = move |_headers: HeaderMap| async move {
            let mut out = HeaderMap::new();
            out.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&small.len().to_string()).unwrap(),
            );
            (StatusCode::OK, out, small.to_vec())
        };

        let redirect = || async {
            (
                StatusCode::FOUND,
                [(header::LOCATION, "/video.mp4".to_string())],
                Vec::<u8>::new(),
            )
        };
        let bad_redirect = || async {
            (
                StatusCode::FOUND,
                [(header::LOCATION, "file:///etc/passwd".to_string())],
                Vec::<u8>::new(),
            )
        };

        let router = Router::new()
            .route("/video.mp4", test_get(range.clone()).head(range))
            .route(
                "/ignores-range.mp4",
                test_get(ignores_range.clone()).head(ignores_range),
            )
            .route(
                "/small.mp4",
                test_get(small_range.clone()).head(small_range),
            )
            .route("/redirect.mp4", test_get(redirect))
            .route("/bad-redirect.mp4", test_get(bad_redirect));
        let listener = TestListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Fixture {
            base: format!("http://127.0.0.1:{port}"),
            seen,
            body,
            _handle: handle,
        }
    }

    type SeenLog = Arc<StdMutex<Vec<(String, HeaderMap)>>>;

    struct HlsFixture {
        base: String,
        seen: SeenLog,
        _handle: tokio::task::JoinHandle<()>,
    }

    impl HlsFixture {
        fn seen_for(&self, needle: &str) -> Option<HeaderMap> {
            self.seen
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|(uri, _)| uri.contains(needle))
                .map(|(_, headers)| headers.clone())
        }
    }

    async fn record_hls(State(seen): State<SeenLog>, request: Request, next: Next) -> Response {
        seen.lock()
            .unwrap()
            .push((request.uri().to_string(), request.headers().clone()));
        next.run(request).await
    }

    fn ranged_response(
        headers: &HeaderMap,
        body: &[u8],
        content_type: &'static str,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut out = HeaderMap::new();
        out.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        let range = headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_range);
        let Some(range) = range else {
            out.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&body.len().to_string()).unwrap(),
            );
            out.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
            return (StatusCode::OK, out, body.to_vec());
        };

        let total = body.len() as u64;
        if total == 0 || range.start >= total {
            return (StatusCode::RANGE_NOT_SATISFIABLE, out, Vec::new());
        }
        let start = range.start;
        let end = range
            .end
            .unwrap_or(total.saturating_sub(1))
            .min(total.saturating_sub(1));
        out.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{total}")).unwrap(),
        );
        out.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&(end - start + 1).to_string()).unwrap(),
        );
        out.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        (
            StatusCode::PARTIAL_CONTENT,
            out,
            body[start as usize..=end as usize].to_vec(),
        )
    }

    async fn hls_fixture() -> HlsFixture {
        const MEDIA_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-VERSION:7\n\
#EXT-X-TARGETDURATION:4\n\
#EXT-X-MEDIA-SEQUENCE:1\n\
#EXT-X-DEFINE:NAME=\"dir\",VALUE=\"seg\"\n\
#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin?auth=1\",IV=0x0123456789abcdef0123456789abcdef\n\
#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"720@0\"\n\
#EXT-X-BYTERANGE:1000@0\n\
seg1.ts\n\
#EXT-X-BYTERANGE:500@1000\n\
{$dir}/seg2.ts?token=abc\n\
#EXT-X-ENDLIST\n";
        const MALFORMED_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
http://[::bad\n\
seg1.ts\n\
#EXT-X-ENDLIST\n";
        const BODY_ONLY_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
seg1.ts\n\
#EXT-X-ENDLIST\n";
        const MASTER_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-STREAM-INF:BANDWIDTH=1280000\n\
low.m3u8\n";
        const BIG_KEY_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
#EXT-X-KEY:METHOD=AES-128,URI=\"bigkey.bin\"\n\
#EXT-X-BYTERANGE:16@0\n\
seg1.ts\n\
#EXT-X-ENDLIST\n";
        const BIG_SUBTITLE_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"subs\",NAME=\"English\",URI=\"bigsubs.vtt\"\n\
#EXT-X-BYTERANGE:16@0\n\
seg1.ts\n\
#EXT-X-ENDLIST\n";

        let seen: SeenLog = Arc::new(StdMutex::new(Vec::new()));

        let playlist = |body: &'static str| {
            move || async move {
                (
                    [(
                        header::CONTENT_TYPE,
                        "application/vnd.apple.mpegurl".to_string(),
                    )],
                    body,
                )
            }
        };
        let body_only = || async {
            (
                [(header::CONTENT_TYPE, "text/plain".to_string())],
                BODY_ONLY_PLAYLIST,
            )
        };
        let large_playlist = {
            let mut body = b"#EXTM3U\n#EXT-X-ENDLIST\n".to_vec();
            body.resize(MAX_PLAYLIST_BYTES + 1, b'#');
            move || {
                let body = body.clone();
                async move {
                    (
                        [(
                            header::CONTENT_TYPE,
                            "application/vnd.apple.mpegurl".to_string(),
                        )],
                        body,
                    )
                }
            }
        };
        let key = || async {
            (
                [(header::CONTENT_TYPE, "application/octet-stream".to_string())],
                vec![0x42u8; 16],
            )
        };
        let big_key = || async {
            (
                [(header::CONTENT_TYPE, "application/octet-stream".to_string())],
                vec![0x42u8; MAX_KEY_BYTES + 1],
            )
        };
        let big_subtitle = || async {
            (
                [(header::CONTENT_TYPE, "text/vtt".to_string())],
                vec![b'a'; MAX_SUBTITLE_BYTES + 1],
            )
        };

        let seg1_body = Arc::new(
            (0..2000)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<u8>>(),
        );
        let seg1 = {
            let body = seg1_body.clone();
            move |headers: HeaderMap| {
                let body = body.clone();
                async move { ranged_response(&headers, body.as_slice(), "video/mp2t") }
            }
        };
        let seg2_body = Arc::new(
            (0..2000)
                .map(|index| (index % 239) as u8)
                .collect::<Vec<u8>>(),
        );
        let seg2 = {
            let body = seg2_body.clone();
            move |headers: HeaderMap| {
                let body = body.clone();
                async move { ranged_response(&headers, body.as_slice(), "video/mp2t") }
            }
        };
        let init_body = Arc::new(
            (0..720)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<u8>>(),
        );
        let init = {
            let body = init_body.clone();
            move |headers: HeaderMap| {
                let body = body.clone();
                async move { ranged_response(&headers, body.as_slice(), "video/mp4") }
            }
        };

        let router = Router::new()
            .route("/hls/media.m3u8", test_get(playlist(MEDIA_PLAYLIST)))
            .route(
                "/hls/malformed.m3u8",
                test_get(playlist(MALFORMED_PLAYLIST)),
            )
            .route("/hls/body-only.m3u8", test_get(body_only))
            .route("/hls/master.m3u8", test_get(playlist(MASTER_PLAYLIST)))
            .route("/hls/large.m3u8", test_get(large_playlist))
            .route("/hls/bigkey.m3u8", test_get(playlist(BIG_KEY_PLAYLIST)))
            .route(
                "/hls/bigsubs.m3u8",
                test_get(playlist(BIG_SUBTITLE_PLAYLIST)),
            )
            .route("/hls/key.bin", test_get(key))
            .route("/hls/bigkey.bin", test_get(big_key))
            .route("/hls/bigsubs.vtt", test_get(big_subtitle))
            .route("/hls/seg1.ts", test_get(seg1.clone()).head(seg1))
            .route("/hls/seg/seg2.ts", test_get(seg2.clone()).head(seg2))
            .route("/hls/init.mp4", test_get(init.clone()).head(init))
            .layer(middleware::from_fn_with_state(seen.clone(), record_hls));

        let listener = TestListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        HlsFixture {
            base: format!("http://127.0.0.1:{port}"),
            seen,
            _handle: handle,
        }
    }

    async fn proxy() -> (ProxyStore, String, tokio::task::JoinHandle<()>) {
        let listener = TestListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let store = ProxyStore::for_tests(port);
        let router = store.router();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (store, format!("http://127.0.0.1:{port}"), handle)
    }

    fn headers() -> HashMap<String, String> {
        HashMap::from([
            ("User-Agent".to_string(), "Kaminari-Test".to_string()),
            ("Referer".to_string(), "https://source.example/".to_string()),
            ("Cookie".to_string(), "sid=abc".to_string()),
            ("Authorization".to_string(), "Bearer secret".to_string()),
            ("X-Secret".to_string(), "drop me".to_string()),
        ])
    }

    #[tokio::test]
    async fn bad_token_unknown_id_and_expired_id() {
        let fixture = fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture(
                "app.kaminari.test",
                &format!("{}/video.mp4", fixture.base),
                None,
            )
            .await
            .unwrap();

        let client = reqwest::Client::new();
        let bad = client
            .get(format!(
                "{base}/s/{}/file?token=00000000000000000000000000000000",
                registered.opaque_id
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(bad.status(), StatusCode::FORBIDDEN);

        let unknown = client
            .get(format!(
                "{base}/s/ffffffffffffffffffffffffffffffff/file?token=x"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);

        store.expire_for_test(&registered.opaque_id);
        let expired = client
            .get(format!(
                "{base}/s/{}/file?token={}",
                registered.opaque_id,
                token_of(&registered)
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(expired.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn healthz_carries_cors_headers() {
        let (_store, base, _handle) = proxy().await;
        let response = reqwest::Client::new()
            .get(format!("{base}/healthz"))
            .header(header::ORIGIN, "http://tauri.localhost")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .unwrap(),
            "Range"
        );
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_EXPOSE_HEADERS)
                .unwrap(),
            "Content-Range, Content-Length, Accept-Ranges"
        );
    }

    #[tokio::test]
    async fn options_returns_204_and_validates_the_token() {
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture("app.kaminari.test", "http://127.0.0.1:9/video.mp4", None)
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let response = client
            .request(reqwest::Method::OPTIONS, proxy_url(&base, &registered))
            .header(header::ORIGIN, "http://tauri.localhost")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .unwrap(),
            "Range"
        );
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_MAX_AGE)
                .unwrap(),
            "600"
        );

        let bad = client
            .request(
                reqwest::Method::OPTIONS,
                format!(
                    "{base}/s/{}/file?token=00000000000000000000000000000000",
                    registered.opaque_id
                ),
            )
            .header(header::ORIGIN, "http://tauri.localhost")
            .send()
            .await
            .unwrap();
        assert_eq!(bad.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            bad.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );

        let unknown = client
            .request(
                reqwest::Method::OPTIONS,
                format!("{base}/s/ffffffffffffffffffffffffffffffff/file?token=x"),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn forwards_only_allowlisted_headers() {
        let fixture = fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture(
                "app.kaminari.test",
                &format!("{}/video.mp4", fixture.base),
                Some(headers()),
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .get(proxy_url(&base, &registered))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let seen = fixture.last_headers();
        assert_eq!(seen.get("user-agent").unwrap(), "Kaminari-Test");
        assert_eq!(seen.get("referer").unwrap(), "https://source.example/");
        assert_eq!(seen.get("cookie").unwrap(), "sid=abc");
        assert_eq!(seen.get("authorization").unwrap(), "Bearer secret");
        assert!(seen.get("x-secret").is_none());
        assert!(seen.get("host").is_some());
    }

    #[tokio::test]
    async fn relays_a_206_and_its_range_headers() {
        let fixture = fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture(
                "app.kaminari.test",
                &format!("{}/video.mp4", fixture.base),
                None,
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .get(proxy_url(&base, &registered))
            .header(header::RANGE, "bytes=1024-2047")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 1024-2047/2097152"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            "1024"
        );
        let bytes = response.bytes().await.unwrap();
        assert_eq!(bytes.as_ref(), &fixture.body[1024..2048]);
        assert_eq!(
            fixture.last_headers().get(header::RANGE).unwrap(),
            "bytes=1024-2047"
        );
    }

    #[tokio::test]
    async fn converts_an_ignored_range_under_the_skip_cap() {
        let fixture = fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture(
                "app.kaminari.test",
                &format!("{}/ignores-range.mp4", fixture.base),
                None,
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .get(proxy_url(&base, &registered))
            .header(header::RANGE, "bytes=1048576-")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 1048576-2097151/2097152"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            "1048576"
        );
        let bytes = response.bytes().await.unwrap();
        assert_eq!(bytes.as_ref(), &fixture.body[1048576..]);
    }

    #[tokio::test]
    async fn refuses_to_skip_past_the_cap() {
        let fixture = fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture(
                "app.kaminari.test",
                &format!("{}/small.mp4", fixture.base),
                None,
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .get(proxy_url(&base, &registered))
            .header(header::RANGE, format!("bytes={}-", MAX_RANGE_SKIP + 1))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.text().await.unwrap(), "range-unsupported");
    }

    #[tokio::test]
    async fn head_relays_headers_without_a_body() {
        let fixture = fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture(
                "app.kaminari.test",
                &format!("{}/video.mp4", fixture.base),
                None,
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .head(proxy_url(&base, &registered))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            "2097152"
        );
        assert!(response.bytes().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn follows_an_allowlisted_redirect_and_rejects_a_file_url() {
        let fixture = fixture().await;
        let (store, base, _handle) = proxy().await;

        let good = store
            .register_fixture(
                "app.kaminari.test",
                &format!("{}/redirect.mp4", fixture.base),
                None,
            )
            .await
            .unwrap();
        let response = reqwest::Client::new()
            .get(proxy_url(&base, &good))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            fixture.body.as_ref()
        );

        let bad = store
            .register_fixture(
                "app.kaminari.test",
                &format!("{}/bad-redirect.mp4", fixture.base),
                None,
            )
            .await
            .unwrap();
        let response = reqwest::Client::new()
            .get(proxy_url(&base, &bad))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn rewrites_a_media_playlist_with_relative_children() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/media.m3u8", fixture.base),
                "hls",
                Some(headers()),
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            PLAYLIST_CONTENT_TYPE
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );

        let text = response.text().await.unwrap();
        assert!(text.starts_with("#EXTM3U"));
        assert!(!text.contains(&fixture.base), "{text}");
        assert!(!text.contains("https://"), "{text}");
        assert!(!text.contains("key.bin"), "{text}");
        assert!(!text.contains("init.mp4"), "{text}");
        assert!(text.contains("#EXT-X-BYTERANGE:1000@0"), "{text}");
        assert!(text.contains("#EXT-X-BYTERANGE:500@1000"), "{text}");

        let key_line = text
            .lines()
            .find(|line| line.starts_with("#EXT-X-KEY"))
            .unwrap();
        assert!(key_line.contains("/s/"), "{key_line}");
        assert!(
            key_line.contains("IV=0x0123456789abcdef0123456789abcdef"),
            "{key_line}"
        );

        let map_line = text
            .lines()
            .find(|line| line.starts_with("#EXT-X-MAP"))
            .unwrap();
        assert!(map_line.contains("/s/"), "{map_line}");
        assert_eq!(seg_paths(&text).len(), 2);
    }

    #[tokio::test]
    async fn hls_children_forward_headers_range_and_query() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/media.m3u8", fixture.base),
                "hls",
                Some(headers()),
            )
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let manifest = client
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let segments = seg_paths(&manifest);
        assert_eq!(segments.len(), 2);

        let response = client
            .get(format!("{base}{}", segments[0]))
            .header(header::RANGE, "bytes=10-19")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.bytes().await.unwrap().len(), 10);
        let seen = fixture.seen_for("/hls/seg1.ts").unwrap();
        assert_eq!(seen.get("referer").unwrap(), "https://source.example/");
        assert_eq!(seen.get("cookie").unwrap(), "sid=abc");
        assert_eq!(seen.get(header::RANGE).unwrap(), "bytes=10-19");

        let response = client
            .get(format!("{base}{}", segments[1]))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 1000-1499/2000"
        );
        let seen = fixture.seen_for("/hls/seg/seg2.ts?token=abc").unwrap();
        assert_eq!(seen.get(header::RANGE).unwrap(), "bytes=1000-1499");

        let key_line = manifest
            .lines()
            .find(|line| line.starts_with("#EXT-X-KEY"))
            .unwrap();
        let key_path = child_path_from_line(key_line).unwrap();
        let response = client
            .get(format!("{base}{key_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.bytes().await.unwrap().as_ref(), &[0x42u8; 16]);
        let seen = fixture.seen_for("/hls/key.bin?auth=1").unwrap();
        assert_eq!(seen.get("cookie").unwrap(), "sid=abc");

        let map_line = manifest
            .lines()
            .find(|line| line.starts_with("#EXT-X-MAP"))
            .unwrap();
        let map_path = child_path_from_line(map_line).unwrap();
        let response = client
            .get(format!("{base}{map_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 0-719/720"
        );
        let seen = fixture.seen_for("/hls/init.mp4").unwrap();
        assert_eq!(seen.get(header::RANGE).unwrap(), "bytes=0-719");
    }

    #[tokio::test]
    async fn hls_child_ids_are_reused_across_refreshes() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/media.m3u8", fixture.base),
                "hls",
                Some(headers()),
            )
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let first = client
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let second = client
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(seg_paths(&first), seg_paths(&second));

        let children = store
            .inner
            .lock()
            .unwrap()
            .entries
            .get(&registered.opaque_id)
            .unwrap()
            .children
            .len();
        assert_eq!(children, 4);
    }

    #[tokio::test]
    async fn hls_malformed_uri_is_dropped() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/malformed.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let response = client
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let text = response.text().await.unwrap();
        assert!(!text.contains("::bad"), "{text}");
        let segments = seg_paths(&text);
        assert_eq!(segments.len(), 1);

        let response = client
            .get(format!("{base}{}", segments[0]))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn hls_master_playlist_is_rejected() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/master.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            response.text().await.unwrap(),
            "master-playlist-unsupported"
        );
    }

    #[tokio::test]
    async fn hls_playlist_without_content_type_is_accepted() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/body-only.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.text().await.unwrap().contains("/seg/"));
    }

    #[tokio::test]
    async fn hls_playlist_over_two_megabytes_is_rejected() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/large.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();

        let response = reqwest::Client::new()
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.text().await.unwrap(), "playlist-too-large");
    }

    #[tokio::test]
    async fn hls_key_and_subtitle_caps_are_enforced() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let client = reqwest::Client::new();

        let big_key = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/bigkey.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();
        let manifest = client
            .get(hls_proxy_url(&base, &big_key))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let key_line = manifest
            .lines()
            .find(|line| line.starts_with("#EXT-X-KEY"))
            .unwrap();
        let key_path = child_path_from_line(key_line).unwrap();
        let response = client
            .get(format!("{base}{key_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.text().await.unwrap(), "key-too-large");

        let big_subtitle = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/bigsubs.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();
        let manifest = client
            .get(hls_proxy_url(&base, &big_subtitle))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let media_line = manifest
            .lines()
            .find(|line| line.starts_with("#EXT-X-MEDIA"))
            .unwrap();
        assert!(media_line.contains("/s/"), "{media_line}");
        let subtitle_path = child_path_from_line(media_line).unwrap();
        let response = client
            .get(format!("{base}{subtitle_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.text().await.unwrap(), "subtitle-too-large");
    }

    #[tokio::test]
    async fn hls_routes_keep_tokens_and_cors() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/media.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let bad = client
            .get(format!(
                "{base}/s/{}/manifest.m3u8?token=00000000000000000000000000000000",
                registered.opaque_id
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(bad.status(), StatusCode::FORBIDDEN);

        let bad_child = client
            .get(format!(
                "{base}/s/{}/seg/ffffffffffffffff?token=00000000000000000000000000000000",
                registered.opaque_id
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(bad_child.status(), StatusCode::FORBIDDEN);

        let unknown_child = client
            .get(format!(
                "{base}/s/{}/seg/ffffffffffffffff?token={}",
                registered.opaque_id,
                token_of(&registered)
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(unknown_child.status(), StatusCode::NOT_FOUND);

        let options = client
            .request(reqwest::Method::OPTIONS, hls_proxy_url(&base, &registered))
            .header(header::ORIGIN, "http://tauri.localhost")
            .send()
            .await
            .unwrap();
        assert_eq!(options.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            options
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );
        assert_eq!(
            options
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .unwrap(),
            "Range"
        );

        let manifest = client
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap();
        assert_eq!(manifest.status(), StatusCode::OK);
        assert_eq!(
            manifest
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );
    }

    #[tokio::test]
    async fn child_registry_reuses_ids_and_caps_children() {
        let store = ProxyStore::for_tests(39321);
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                "http://127.0.0.1:9/media.m3u8",
                "hls",
                None,
            )
            .await
            .unwrap();

        let specs = (0..10)
            .map(|index| ChildSpec {
                upstream_url: format!("http://127.0.0.1:9/seg/{index}.ts"),
                content_type: None,
                byte_range: None,
                kind: ChildKind::Segment,
            })
            .collect::<Vec<_>>();
        let first = store
            .reconcile_children(&registered.opaque_id, &specs)
            .unwrap();
        let second = store
            .reconcile_children(&registered.opaque_id, &specs)
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            store
                .inner
                .lock()
                .unwrap()
                .entries
                .get(&registered.opaque_id)
                .unwrap()
                .children
                .len(),
            10
        );

        let duplicate = ChildSpec {
            upstream_url: "http://127.0.0.1:9/seg/dup.ts".to_string(),
            content_type: None,
            byte_range: None,
            kind: ChildKind::Segment,
        };
        let ids = store
            .reconcile_children(&registered.opaque_id, &[duplicate.clone(), duplicate])
            .unwrap();
        assert_eq!(ids[0], ids[1]);

        let many = (0..MAX_CHILDREN + 40)
            .map(|index| ChildSpec {
                upstream_url: format!("http://127.0.0.1:9/many/{index}.ts"),
                content_type: None,
                byte_range: None,
                kind: ChildKind::Segment,
            })
            .collect::<Vec<_>>();
        let ids = store
            .reconcile_children(&registered.opaque_id, &many)
            .unwrap();
        assert_eq!(ids.len(), MAX_CHILDREN + 40);
        assert_eq!(
            store
                .inner
                .lock()
                .unwrap()
                .entries
                .get(&registered.opaque_id)
                .unwrap()
                .children
                .len(),
            MAX_CHILDREN
        );
    }

    #[test]
    fn resolves_relative_playlist_urls_and_hides_the_origin() {
        let base = Url::parse("https://cdn.example/hls/media.m3u8").unwrap();
        let text = "#EXTM3U\n\
#EXT-X-DEFINE:NAME=\"dir\",VALUE=\"seg\"\n\
#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0xABCD\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"en\",URI=\"audio.m3u8\"\n\
{$dir}/1.ts?token=abc\n";
        let parsed = parse_media_playlist(&base, text).unwrap();
        assert_eq!(parsed.specs.len(), 3);
        assert_eq!(
            parsed.specs[0].upstream_url,
            "https://cdn.example/hls/key.bin"
        );
        assert_eq!(
            parsed.specs[1].upstream_url,
            "https://cdn.example/hls/audio.m3u8"
        );
        assert_eq!(
            parsed.specs[2].upstream_url,
            "https://cdn.example/hls/seg/1.ts?token=abc"
        );

        let entry = test_entry();
        let child_ids = vec![
            "key".to_string(),
            "audio".to_string(),
            "segment".to_string(),
        ];
        let rendered = render_playlist(&entry, &parsed, &child_ids);
        assert!(!rendered.contains("https://cdn.example"), "{rendered}");
        assert!(rendered.contains("IV=0xABCD"), "{rendered}");
        assert!(
            rendered.contains("/s/opaque/seg/key?token=token"),
            "{rendered}"
        );
        assert!(
            rendered.contains("/s/opaque/seg/segment?token=token"),
            "{rendered}"
        );
        assert_eq!(rendered.matches("/s/opaque/seg/").count(), 3);
    }

    fn test_entry() -> ProxyEntry {
        let now = Instant::now();
        ProxyEntry {
            opaque_id: "opaque".to_string(),
            token: "token".to_string(),
            extension_id: "app.kaminari.test".to_string(),
            upstream_url: "https://cdn.example/hls/media.m3u8".to_string(),
            kind: "hls".to_string(),
            headers: Vec::new(),
            allowed: None,
            allow_private: true,
            created_at: now,
            last_used: now,
            children: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn blocks_private_and_metadata_upstreams_at_register() {
        let store = ProxyStore::with_state(39321, true, false);
        let rules = vec![
            HostRule::parse("https://127.0.0.1/*").unwrap(),
            HostRule::parse("https://10.0.0.1/*").unwrap(),
            HostRule::parse("https://169.254.169.254/*").unwrap(),
        ];
        for url in [
            "https://127.0.0.1/video.mp4",
            "https://10.0.0.1/video.mp4",
            "https://169.254.169.254/latest/meta-data",
        ] {
            let stream = ProxyStreamInput {
                url: url.to_string(),
                kind: "mp4".to_string(),
                headers: None,
            };
            let error = store
                .register("app.kaminari.test", &stream, rules.clone())
                .await
                .unwrap_err();
            assert!(
                error.contains("not public") || error.contains("allowlist"),
                "{url}: {error}"
            );
        }
    }

    #[test]
    fn header_sanitizer_rejects_control_characters_and_caps() {
        let injection = HashMap::from([(
            "Referer".to_string(),
            "https://example.com/\r\nX-Evil: 1".to_string(),
        )]);
        assert!(sanitize_headers(Some(&injection)).is_err());

        let too_many = (0..17)
            .map(|index| (format!("X-{index}"), "1".to_string()))
            .collect::<HashMap<_, _>>();
        assert!(sanitize_headers(Some(&too_many)).is_err());

        let dropped = HashMap::from([
            ("Host".to_string(), "evil".to_string()),
            ("Proxy-Authorization".to_string(), "secret".to_string()),
            ("User-Agent".to_string(), "kept".to_string()),
        ]);
        let kept = sanitize_headers(Some(&dropped)).unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].0, header::USER_AGENT);
    }

    #[test]
    fn blocks_private_and_mapped_addresses() {
        for raw in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.1.1",
            "169.254.169.254",
            "224.0.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
        ] {
            assert!(is_blocked_ip(raw.parse().unwrap()), "{raw}");
        }
        assert!(!is_blocked_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_blocked_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn csp_loopback_matches_bind_set() {
        let raw = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"))
            .unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        let csp = config["app"]["security"]["csp"].as_str().unwrap();
        let expected = PORTS
            .iter()
            .map(|port| format!("http://127.0.0.1:{port}"))
            .collect::<Vec<_>>();

        for directive in ["connect-src", "media-src"] {
            let mut loopbacks = csp_directive(csp, directive)
                .into_iter()
                .filter(|token| {
                    token.contains("127.0.0.1") || token.starts_with("http://localhost")
                })
                .collect::<Vec<_>>();
            loopbacks.sort();
            let mut expected = expected.clone();
            expected.sort();
            assert_eq!(loopbacks, expected, "{directive} loopback origins");
        }

        assert!(!csp.contains("127.0.0.1:*"));
        assert!(!csp.to_ascii_lowercase().contains("localhost:"));
        for directive in csp.split(';') {
            let Some((name, rest)) = directive.trim().split_once(' ') else {
                continue;
            };
            if name == "connect-src" || name == "media-src" {
                continue;
            }
            assert!(
                !rest.contains("127.0.0.1")
                    && !rest
                        .split_whitespace()
                        .any(|token| token.starts_with("http://localhost")),
                "{name} must not widen to loopback"
            );
        }
    }

    fn csp_directive(csp: &str, name: &str) -> Vec<String> {
        csp.split(';')
            .map(str::trim)
            .find(|part| part.split_whitespace().next() == Some(name))
            .map(|part| {
                part.split_whitespace()
                    .skip(1)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn token_of(registered: &ProxyUrl) -> String {
        registered
            .proxy_url
            .split("token=")
            .nth(1)
            .unwrap_or_default()
            .to_string()
    }

    fn hls_proxy_url(base: &str, registered: &ProxyUrl) -> String {
        format!(
            "{base}/s/{}/manifest.m3u8?token={}",
            registered.opaque_id,
            token_of(registered)
        )
    }

    fn child_path_from_line(line: &str) -> Option<String> {
        let start = line.find("/s/")?;
        let rest = &line[start..];
        let end = rest.find(['"', ' ', ',']).unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }

    fn seg_paths(text: &str) -> Vec<String> {
        text.lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| child_path_from_line(line.trim()))
            .collect()
    }

    fn proxy_url(base: &str, registered: &ProxyUrl) -> String {
        format!(
            "{base}/s/{}/file?token={}",
            registered.opaque_id,
            token_of(registered)
        )
    }
}
