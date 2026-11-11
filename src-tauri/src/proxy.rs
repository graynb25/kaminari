// Loopback stream proxy for protected MP4 and HLS playback.
// See docs/PLAN-STREAM-PROXY.md. The WebView gets an opaque loopback URL.
// Rust keeps the upstream URL, the allowlisted headers, the per entry token
// and the rewritten playlist children in memory only.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

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
// Keep the child cap at 4096: an earlier 256 child cap broke long VOD
// playlists (see docs/ENGINEERING-NOTES.md). The cap is enforced before a
// refresh mutates the table, so a playlist can never leave dangling child ids.
pub const MAX_CHILDREN: usize = 4096;
pub const MAX_PLAYLIST_BYTES: usize = 2 * 1024 * 1024;
// Rewriting each child URI expands it to a loopback path. A 2 MB playlist can
// grow by roughly one proxy path per segment line, so bound the rendered body
// at twice the upstream cap.
pub const MAX_RENDERED_PLAYLIST_BYTES: usize = 2 * MAX_PLAYLIST_BYTES;
pub const MAX_PLAYLIST_DEPTH: u8 = 4;
pub const MAX_KEY_BYTES: usize = 64 * 1024;
pub const MAX_SUBTITLE_BYTES: usize = 512 * 1024;
pub const MAX_COOKIES: usize = 32;
pub const MAX_COOKIE_BYTES: usize = 4 * 1024;
pub const MAX_RANGE_SKIP: u64 = 16 * 1024 * 1024;
pub const STREAM_CHUNK_BYTES: usize = 64 * 1024;
pub const SWEEP_SECS: u64 = 60;

const MAX_REDIRECTS: usize = 3;
const MAX_PLAYLIST_LINES: usize = 20_000;
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
    upstream_url: String,
    content_type: Option<String>,
    byte_range: Option<String>,
    kind: ChildKind,
    depth: u8,
    last_used: Instant,
}

#[derive(Debug, Clone)]
struct ChildSpec {
    upstream_url: String,
    content_type: Option<String>,
    byte_range: Option<String>,
    kind: ChildKind,
    depth: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SameSite {
    Strict,
    Lax,
    None,
}

#[derive(Debug, Clone)]
struct JarCookie {
    name: String,
    value: String,
    domain: String,
    host_only: bool,
    path: String,
    expires_at: Option<SystemTime>,
    secure: bool,
    same_site: Option<SameSite>,
    created_at: Instant,
}

#[derive(Debug, Clone)]
struct SeedCookie {
    name: String,
    value: String,
    host: String,
    path: String,
}

#[derive(Debug, Default)]
struct CookieJar {
    cookies: Vec<JarCookie>,
    seed: Vec<SeedCookie>,
}

impl JarCookie {
    fn expired(&self, now: SystemTime) -> bool {
        self.expires_at.is_some_and(|expires| expires <= now)
    }
}

impl CookieJar {
    fn seeded(url: &Url, values: &[String]) -> Self {
        let mut jar = Self::default();
        let Some(host) = url.host_str() else {
            return jar;
        };
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        for value in values {
            for pair in value.split(';') {
                let Some((name, value)) = pair.split_once('=') else {
                    continue;
                };
                let name = name.trim();
                let value = value.trim();
                if !valid_cookie_name(name) || name.len() + value.len() > MAX_COOKIE_BYTES {
                    continue;
                }
                if jar.seed.iter().any(|cookie| cookie.name == name) {
                    continue;
                }
                jar.seed.push(SeedCookie {
                    name: name.to_string(),
                    value: value.to_string(),
                    host: host.clone(),
                    path: "/".to_string(),
                });
            }
        }
        jar
    }

    fn capture(&mut self, headers: &HeaderMap, request_url: &Url) {
        let now = SystemTime::now();
        self.sweep(now);
        for value in headers.get_all(header::SET_COOKIE).iter() {
            let Ok(raw) = value.to_str() else {
                continue;
            };
            let Some(cookie) = parse_set_cookie(raw, request_url, now) else {
                continue;
            };
            self.store(cookie, now);
        }
    }

    fn header_for(&mut self, url: &Url) -> Option<String> {
        let now = SystemTime::now();
        self.sweep(now);
        let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
        let request_path = url.path();
        let secure = url.scheme() == "https";

        let mut matched = self
            .cookies
            .iter()
            .filter(|cookie| {
                !cookie.expired(now)
                    && domain_matches(&host, &cookie.domain, cookie.host_only)
                    && path_matches(request_path, &cookie.path)
                    && (!cookie.secure || secure)
            })
            .collect::<Vec<_>>();
        matched.sort_by(|left, right| {
            right
                .path
                .len()
                .cmp(&left.path.len())
                .then_with(|| left.created_at.cmp(&right.created_at))
        });

        let mut names = HashSet::new();
        let mut parts = Vec::new();
        for cookie in matched {
            if names.insert(cookie.name.clone()) {
                parts.push(format!("{}={}", cookie.name, cookie.value));
            }
        }
        for cookie in &self.seed {
            if cookie.host != host || !path_matches(request_path, &cookie.path) {
                continue;
            }
            if self.cookies.iter().any(|jar| jar.name == cookie.name)
                || !names.insert(cookie.name.clone())
            {
                continue;
            }
            parts.push(format!("{}={}", cookie.name, cookie.value));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        }
    }

    fn sweep(&mut self, now: SystemTime) {
        self.cookies.retain(|cookie| !cookie.expired(now));
    }

    fn store(&mut self, cookie: JarCookie, now: SystemTime) {
        self.cookies.retain(|existing| {
            !(existing.name == cookie.name
                && existing.domain == cookie.domain
                && existing.path == cookie.path)
        });
        if cookie.expired(now) {
            return;
        }
        tracing::debug!(
            name = %cookie.name,
            same_site = ?cookie.same_site,
            "proxy: stored an upstream cookie"
        );
        self.cookies.push(cookie);
        while self.cookies.len() > MAX_COOKIES {
            if let Some(index) = self
                .cookies
                .iter()
                .enumerate()
                .min_by_key(|(_, cookie)| cookie.created_at)
                .map(|(index, _)| index)
            {
                let dropped = self.cookies.remove(index);
                tracing::warn!(
                    name = %dropped.name,
                    limit = MAX_COOKIES,
                    "proxy: cookie jar is full, dropped the oldest cookie"
                );
            } else {
                break;
            }
        }
    }
}

fn parse_set_cookie(raw: &str, request_url: &Url, now: SystemTime) -> Option<JarCookie> {
    if raw.len() > MAX_COOKIE_BYTES {
        tracing::debug!(
            limit = MAX_COOKIE_BYTES,
            "proxy: dropped an oversized cookie"
        );
        return None;
    }
    let mut parts = raw.split(';');
    let pair = parts.next()?.trim();
    let (name, value) = pair.split_once('=')?;
    let name = name.trim();
    let value = value.trim();
    if !valid_cookie_name(name) || name.len() + value.len() > MAX_COOKIE_BYTES {
        tracing::debug!("proxy: dropped a cookie with an invalid name or size");
        return None;
    }
    let host = request_url
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();

    let mut domain_attr: Option<String> = None;
    let mut path_attr: Option<String> = None;
    let mut max_age: Option<i64> = None;
    let mut expires: Option<SystemTime> = None;
    let mut secure = false;
    let mut same_site = None;

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, attr) = match part.split_once('=') {
            Some((key, attr)) => (key.trim(), attr.trim()),
            None => (part, ""),
        };
        if key.eq_ignore_ascii_case("domain") && !attr.is_empty() {
            domain_attr = Some(attr.to_string());
        } else if key.eq_ignore_ascii_case("path") && attr.starts_with('/') {
            path_attr = Some(attr.to_string());
        } else if key.eq_ignore_ascii_case("max-age") {
            if let Ok(seconds) = attr.parse::<i64>() {
                max_age = Some(seconds);
            }
        } else if key.eq_ignore_ascii_case("expires") {
            expires = httpdate::parse_http_date(attr).ok();
        } else if key.eq_ignore_ascii_case("secure") {
            secure = true;
        } else if key.eq_ignore_ascii_case("samesite") {
            if attr.eq_ignore_ascii_case("strict") {
                same_site = Some(SameSite::Strict);
            } else if attr.eq_ignore_ascii_case("lax") {
                same_site = Some(SameSite::Lax);
            } else if attr.eq_ignore_ascii_case("none") {
                same_site = Some(SameSite::None);
            }
        }
    }

    if same_site == Some(SameSite::None) && !secure {
        tracing::warn!(
            name = %name,
            "proxy: dropped SameSite=None cookie without Secure"
        );
        return None;
    }

    let (domain, host_only) = match domain_attr {
        Some(attr) => {
            // RFC 6265 subset. A public suffix list is deferred, so the
            // embedded dot check only blocks single label supercookies such
            // as Domain=com. A domain cookie must still sit on a label
            // boundary of the request host, never match as a substring.
            let Some(domain) = normalize_cookie_domain(&attr) else {
                tracing::debug!(domain = %attr, "proxy: dropped a cookie with an invalid domain");
                return None;
            };
            if !domain_matches(&host, &domain, false) {
                tracing::debug!(
                    domain = %domain,
                    host = %host,
                    "proxy: dropped a cookie whose domain does not match the request host"
                );
                return None;
            }
            (domain, false)
        }
        None => (host, true),
    };

    let path = path_attr.unwrap_or_else(|| default_cookie_path(request_url));
    let expires_at = match max_age {
        Some(seconds) if seconds <= 0 => now
            .checked_sub(Duration::from_secs(1))
            .or(Some(SystemTime::UNIX_EPOCH)),
        Some(seconds) => now
            .checked_add(Duration::from_secs(seconds as u64))
            .or_else(|| now.checked_add(Duration::from_secs(60 * 60 * 24 * 365 * 100))),
        None => expires,
    };

    Some(JarCookie {
        name: name.to_string(),
        value: value.to_string(),
        domain,
        host_only,
        path,
        expires_at,
        secure,
        same_site,
        created_at: Instant::now(),
    })
}

fn valid_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte > 0x20 && byte < 0x7f && !matches!(byte, b';' | b'=' | b','))
}

fn normalize_cookie_domain(attr: &str) -> Option<String> {
    let domain = attr.trim().trim_start_matches('.').to_ascii_lowercase();
    if domain.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || !domain.contains('.')
        || domain.split('.').any(str::is_empty)
    {
        return None;
    }
    Some(domain)
}

fn domain_matches(host: &str, cookie_domain: &str, host_only: bool) -> bool {
    if host_only {
        return host == cookie_domain;
    }
    host == cookie_domain || host.ends_with(&format!(".{cookie_domain}"))
}

fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    if request_path == cookie_path {
        return true;
    }
    if !request_path.starts_with(cookie_path) {
        return false;
    }
    cookie_path.ends_with('/') || request_path.as_bytes().get(cookie_path.len()) == Some(&b'/')
}

fn default_cookie_path(url: &Url) -> String {
    let path = url.path();
    if !path.starts_with('/') || path == "/" {
        return "/".to_string();
    }
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(index) => path[..index].to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ChildKey {
    upstream_url: String,
    byte_range: Option<String>,
}

impl ChildKey {
    fn from_spec(spec: &ChildSpec) -> Self {
        Self {
            upstream_url: spec.upstream_url.clone(),
            byte_range: spec.byte_range.clone(),
        }
    }

    fn from_child(child: &ChildEntry) -> Self {
        Self {
            upstream_url: child.upstream_url.clone(),
            byte_range: child.byte_range.clone(),
        }
    }
}

#[derive(Default)]
struct ChildIndex {
    by_id: HashMap<String, ChildEntry>,
    by_key: HashMap<ChildKey, String>,
    order: VecDeque<String>,
}

impl ChildIndex {
    fn new() -> Self {
        Self::default()
    }

    fn get_mut(&mut self, child_id: &str) -> Option<&mut ChildEntry> {
        self.by_id.get_mut(child_id)
    }

    fn id_for(&self, key: &ChildKey) -> Option<&str> {
        self.by_key.get(key).map(String::as_str)
    }

    fn insert(&mut self, spec: &ChildSpec, now: Instant) -> Result<String, ProxyError> {
        if self.by_id.len() >= MAX_CHILDREN {
            return Err(ProxyError::Oversized("playlist-too-many-children"));
        }
        let mut child_id = random_hex_n(8).map_err(|_| ProxyError::Upstream("random"))?;
        while self.by_id.contains_key(&child_id) {
            child_id = random_hex_n(8).map_err(|_| ProxyError::Upstream("random"))?;
        }
        let key = ChildKey::from_spec(spec);
        let entry = ChildEntry {
            upstream_url: spec.upstream_url.clone(),
            content_type: spec.content_type.clone(),
            byte_range: spec.byte_range.clone(),
            kind: spec.kind,
            depth: spec.depth,
            last_used: now,
        };
        self.by_key.insert(key, child_id.clone());
        self.by_id.insert(child_id.clone(), entry);
        self.order.push_back(child_id.clone());
        Ok(child_id)
    }

    /// Drop the least recently inserted children that the current playlist
    /// does not reference. Children that are still in the live window keep
    /// their ids, so reconciliation cannot evict a URL it just handed out.
    fn make_room(&mut self, current: &HashSet<ChildKey>) -> Result<(), ProxyError> {
        let existing_current = current
            .iter()
            .filter(|key| self.by_key.contains_key(*key))
            .count();
        let projected = self
            .by_id
            .len()
            .saturating_sub(existing_current)
            .saturating_add(current.len());
        if projected <= MAX_CHILDREN {
            return Ok(());
        }
        let to_evict = projected - MAX_CHILDREN;
        let non_current = self.by_id.len().saturating_sub(existing_current);
        if non_current < to_evict {
            return Err(ProxyError::Oversized("playlist-too-many-children"));
        }

        let mut kept = VecDeque::with_capacity(self.by_id.len());
        let mut evicted = 0usize;
        while let Some(child_id) = self.order.pop_front() {
            let Some(child) = self.by_id.get(&child_id) else {
                continue;
            };
            let key = ChildKey::from_child(child);
            if evicted < to_evict && !current.contains(&key) {
                self.by_id.remove(&child_id);
                self.by_key.remove(&key);
                evicted += 1;
            } else {
                kept.push_back(child_id);
            }
        }
        self.order = kept;
        if self.by_id.len().saturating_sub(existing_current) + current.len() > MAX_CHILDREN {
            return Err(ProxyError::Oversized("playlist-too-many-children"));
        }
        Ok(())
    }
}

struct ProxyEntry {
    opaque_id: String,
    token: String,
    extension_id: String,
    upstream_url: String,
    kind: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    jar: Arc<Mutex<CookieJar>>,
    allowed: Option<Vec<HostRule>>,
    allow_private: bool,
    created_at: Instant,
    last_used: Mutex<Instant>,
    children: Mutex<ChildIndex>,
}

impl ProxyEntry {
    fn touch(&self, now: Instant) {
        if let Ok(mut last_used) = self.last_used.lock() {
            *last_used = now;
        }
    }

    fn last_used(&self) -> Instant {
        *self
            .last_used
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn cookie_header(&self, url: &Url) -> Option<String> {
        self.jar.lock().ok().and_then(|mut jar| jar.header_for(url))
    }

    fn capture_cookies(&self, headers: &HeaderMap, url: &Url) {
        if let Ok(mut jar) = self.jar.lock() {
            jar.capture(headers, url);
        }
    }
}

struct StoreState {
    entries: HashMap<String, Arc<ProxyEntry>>,
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
    PlaylistDepth,
    ChildNotAllowed,
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

#[derive(Clone, Copy)]
struct PlaylistOrigin<'a> {
    base: &'a Url,
    depth: u8,
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

        let mut headers = sanitize_headers(stream.headers.as_ref())?;
        let seed = headers
            .iter()
            .filter(|(name, _)| *name == header::COOKIE)
            .filter_map(|(_, value)| value.to_str().ok().map(str::to_string))
            .collect::<Vec<_>>();
        headers.retain(|(name, _)| *name != header::COOKIE);
        let jar = CookieJar::seeded(&url, &seed);
        let allow_private = allow_private || store_allows_private;
        resolve_checked(&url, allow_private).await?;

        let opaque_id = random_hex()?;
        let token = random_hex()?;
        let now = Instant::now();
        let kind = stream.kind.to_ascii_lowercase();
        let entry = Arc::new(ProxyEntry {
            opaque_id: opaque_id.clone(),
            token: token.clone(),
            extension_id: extension_id.to_string(),
            upstream_url: url.to_string(),
            kind: kind.clone(),
            headers,
            jar: Arc::new(Mutex::new(jar)),
            allowed,
            allow_private,
            created_at: now,
            last_used: Mutex::new(now),
            children: Mutex::new(ChildIndex::new()),
        });

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
                    .min_by_key(|(_, entry)| entry.last_used())
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

    /// True only for a URL the app's proxy issued for a live entry: loopback
    /// host, this store's port, `/s/{opaque_id}/...` and the matching random
    /// token. Downloads use this to accept the proxy without accepting an
    /// arbitrary localhost URL.
    pub fn authorizes_url(&self, value: &str) -> bool {
        let Ok(url) = Url::parse(value) else {
            return false;
        };
        if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
            return false;
        }
        if !url.username().is_empty() || url.password().is_some() {
            return false;
        }
        let Ok(state) = self.inner.lock() else {
            return false;
        };
        if !state.enabled || url.port() != Some(state.port) {
            return false;
        }
        let segments: Vec<&str> = url
            .path_segments()
            .map(|segments| segments.collect())
            .unwrap_or_default();
        if segments.len() < 3 || segments[0] != "s" {
            return false;
        }
        let Some(entry) = state.entries.get(segments[1]).cloned() else {
            return false;
        };
        drop(state);
        let now = Instant::now();
        if now.duration_since(entry.created_at) >= ABSOLUTE_TTL
            || now.duration_since(entry.last_used()) >= IDLE_TTL
        {
            return false;
        }
        let token = url
            .query_pairs()
            .find(|(name, _)| name == "token")
            .map(|(_, value)| value.into_owned());
        matches!(token.as_deref(), Some(token) if constant_time_eq(token.as_bytes(), entry.token.as_bytes()))
    }

    fn lookup(&self, id: &str, token: &str) -> Result<Arc<ProxyEntry>, LookupError> {
        let entry = {
            let state = self.inner.lock().map_err(|_| LookupError::NotFound)?;
            state
                .entries
                .get(id)
                .cloned()
                .ok_or(LookupError::NotFound)?
        };
        let now = Instant::now();
        check_entry(&entry, token, now)?;
        entry.touch(now);
        Ok(entry)
    }

    fn lookup_child(
        &self,
        id: &str,
        token: &str,
        child_id: &str,
    ) -> Result<(Arc<ProxyEntry>, ChildEntry), LookupError> {
        let entry = {
            let state = self.inner.lock().map_err(|_| LookupError::NotFound)?;
            state
                .entries
                .get(id)
                .cloned()
                .ok_or(LookupError::NotFound)?
        };
        let now = Instant::now();
        check_entry(&entry, token, now)?;
        entry.touch(now);
        let child = {
            let mut children = entry.children.lock().map_err(|_| LookupError::NotFound)?;
            let child = children.get_mut(child_id).ok_or(LookupError::NotFound)?;
            child.last_used = now;
            child.clone()
        };
        Ok((entry, child))
    }

    fn reconcile_children(&self, id: &str, specs: &[ChildSpec]) -> Result<Vec<String>, ProxyError> {
        // Bound the work and the table before touching the entry. A playlist
        // that needs more distinct children than the cap fails closed instead
        // of leaving loopback URLs that point at evicted children.
        let mut current = HashSet::with_capacity(specs.len().min(MAX_CHILDREN + 1));
        for spec in specs {
            current.insert(ChildKey::from_spec(spec));
            if current.len() > MAX_CHILDREN {
                return Err(ProxyError::Oversized("playlist-too-many-children"));
            }
        }

        let entry = {
            let state = self
                .inner
                .lock()
                .map_err(|_| ProxyError::Upstream("store"))?;
            state
                .entries
                .get(id)
                .cloned()
                .ok_or(ProxyError::Upstream("entry"))?
        };
        let mut children = entry
            .children
            .lock()
            .map_err(|_| ProxyError::Upstream("store"))?;
        children.make_room(&current)?;

        let now = Instant::now();
        let mut ids = Vec::with_capacity(specs.len());
        for spec in specs {
            let key = ChildKey::from_spec(spec);
            if let Some(child_id) = children.id_for(&key).map(str::to_string) {
                if let Some(child) = children.get_mut(&child_id) {
                    if spec.content_type.is_some() {
                        child.content_type = spec.content_type.clone();
                    }
                    child.byte_range = spec.byte_range.clone();
                    child.kind = spec.kind;
                    child.depth = spec.depth;
                    child.last_used = now;
                    ids.push(child_id);
                    continue;
                }
                // Defensive: drop an index entry that lost its child.
                children.by_key.remove(&key);
            }
            let child_id = children.insert(spec, now)?;
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
                || now.duration_since(entry.last_used()) >= IDLE_TTL;
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
    pub(crate) fn for_tests(port: u16) -> Self {
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
            if let Ok(mut last_used) = entry.last_used.lock() {
                *last_used = Instant::now() - IDLE_TTL - Duration::from_secs(1);
            }
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
    // A live playlist (no EXT-X-ENDLIST) must be fetched again on every
    // request. There is no playlist cache. Child ids are reconciled below by
    // upstream URL plus byte range, so overlapping live windows keep their ids.
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
    let parsed = match parse_playlist(&final_url, &text, 0) {
        Ok(parsed) => parsed,
        Err(err) => return proxy_error_response(err),
    };
    if let Err(err) = check_specs_allowed(&entry, &parsed.specs) {
        return proxy_error_response(err);
    }
    let child_ids = match store.reconcile_children(&id, &parsed.specs) {
        Ok(ids) => ids,
        Err(err) => return proxy_error_response(err),
    };
    let rewritten = match render_playlist(&entry, &parsed, &child_ids) {
        Ok(rewritten) => rewritten,
        Err(err) => return proxy_error_response(err),
    };
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
    if child.kind == ChildKind::Playlist && child.depth > MAX_PLAYLIST_DEPTH {
        return proxy_error_response(ProxyError::PlaylistDepth);
    }
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
    if child.depth > MAX_PLAYLIST_DEPTH {
        return proxy_error_response(ProxyError::PlaylistDepth);
    }
    let text = String::from_utf8_lossy(&body);
    let parsed = match parse_playlist(&final_url, &text, child.depth) {
        Ok(parsed) => parsed,
        Err(err) => return proxy_error_response(err),
    };
    if let Err(err) = check_specs_allowed(entry, &parsed.specs) {
        return proxy_error_response(err);
    }
    let child_ids = match store.reconcile_children(&entry.opaque_id, &parsed.specs) {
        Ok(ids) => ids,
        Err(err) => return proxy_error_response(err),
    };
    let rewritten = match render_playlist(entry, &parsed, &child_ids) {
        Ok(rewritten) => rewritten,
        Err(err) => return proxy_error_response(err),
    };
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
    // A playlist byte range defines the exact bytes of this child. hls.js can
    // send its own Range, but the registered range wins for that child URL.
    let effective = match child.byte_range.as_deref() {
        Some(range) => Some(format!("bytes={range}")),
        None => requested,
    };
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
            if *name == header::COOKIE {
                // The extension cookie was seeded into the jar once at
                // register. It is never forwarded raw on each request.
                continue;
            }
            if !same_origin && *name == header::AUTHORIZATION {
                continue;
            }
            request = request.header(name, value);
        }
        if let Some(value) = entry.cookie_header(&current) {
            request = request.header(header::COOKIE, value);
        }
        if let Some(value) = range {
            request = request.header(header::RANGE, value);
        }

        let response = tokio::time::timeout(FIRST_BYTE_TIMEOUT, request.send())
            .await
            .map_err(|_| ProxyError::Timeout)?
            .map_err(|_| ProxyError::Upstream("send"))?;

        // Capture before the redirect check so a Set-Cookie on a 3xx is not
        // lost. The jar scopes it to the request that set it.
        entry.capture_cookies(response.headers(), &current);

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
        ProxyError::PlaylistDepth => short(StatusCode::BAD_GATEWAY, "playlist-depth"),
        ProxyError::ChildNotAllowed => short(StatusCode::BAD_GATEWAY, "child-not-allowed"),
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

fn check_entry(entry: &ProxyEntry, token: &str, now: Instant) -> Result<(), LookupError> {
    if !constant_time_eq(entry.token.as_bytes(), token.as_bytes()) {
        return Err(LookupError::Forbidden);
    }
    if now.duration_since(entry.created_at) >= ABSOLUTE_TTL
        || now.duration_since(entry.last_used()) >= IDLE_TTL
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

fn parse_playlist(base: &Url, text: &str, depth: u8) -> Result<ParsedPlaylist, ProxyError> {
    let mut lines = Vec::new();
    let mut specs = Vec::new();
    let mut defines: HashMap<String, String> = HashMap::new();
    let mut pending_range: Option<String> = None;
    let mut next_offset = 0u64;
    let mut count = 0usize;
    let mut pending_variant = false;
    let child_depth = depth.saturating_add(1);
    let origin = PlaylistOrigin {
        base,
        depth: child_depth,
    };

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
        if pending_variant && !line.starts_with('#') {
            pending_variant = false;
            let Some(uri) = substitute_defines(line, &defines) else {
                tracing::debug!(
                    tag = "stream-inf",
                    "proxy: dropped a variant URI with an undefined variable"
                );
                continue;
            };
            let Some(resolved) = resolve_playlist_url(base, &uri) else {
                tracing::debug!(tag = "stream-inf", "proxy: dropped a malformed variant URI");
                continue;
            };
            let spec = ChildSpec {
                upstream_url: resolved.to_string(),
                content_type: ChildKind::Playlist
                    .default_content_type()
                    .map(str::to_string),
                byte_range: None,
                kind: ChildKind::Playlist,
                depth: child_depth,
            };
            let index = specs.len();
            specs.push(spec);
            lines.push(PlaylistLine::Segment { spec: index });
            continue;
        }
        if line.starts_with("#EXT-X-STREAM-INF") {
            pending_range = None;
            if find_attr(line, "URI").is_some() {
                pending_variant = false;
                push_uri_tag(
                    line,
                    ChildKind::Playlist,
                    None,
                    &defines,
                    origin,
                    &mut lines,
                    &mut specs,
                );
            } else {
                lines.push(PlaylistLine::Text(line.to_string()));
                pending_variant = true;
            }
            continue;
        }
        if line.starts_with("#EXT-X-I-FRAME-STREAM-INF") {
            pending_range = None;
            push_uri_tag(
                line,
                ChildKind::Playlist,
                None,
                &defines,
                origin,
                &mut lines,
                &mut specs,
            );
            continue;
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
                    origin,
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
                origin,
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
            push_uri_tag(line, kind, None, &defines, origin, &mut lines, &mut specs);
            continue;
        }
        if line.starts_with("#EXT-X-SESSION-DATA") {
            push_uri_tag(
                line,
                ChildKind::File,
                None,
                &defines,
                origin,
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
                origin,
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
                origin,
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
            depth: child_depth,
        };
        let index = specs.len();
        specs.push(spec);
        lines.push(PlaylistLine::Segment { spec: index });
    }

    Ok(ParsedPlaylist { lines, specs })
}

fn check_specs_allowed(entry: &ProxyEntry, specs: &[ChildSpec]) -> Result<(), ProxyError> {
    let Some(rules) = entry.allowed.as_ref() else {
        return Ok(());
    };
    for spec in specs {
        let Ok(url) = Url::parse(&spec.upstream_url) else {
            return Err(ProxyError::ChildNotAllowed);
        };
        if !rules.iter().any(|rule| rule.allows(&url)) {
            tracing::debug!(host = %url.host_str().unwrap_or(""), "proxy: child is not allowlisted");
            return Err(ProxyError::ChildNotAllowed);
        }
    }
    Ok(())
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
    origin: PlaylistOrigin<'_>,
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
    let Some(resolved) = resolve_playlist_url(origin.base, &uri) else {
        tracing::debug!(tag = %tag_name(line), "proxy: dropped a malformed playlist URI");
        return;
    };
    let spec = ChildSpec {
        upstream_url: resolved.to_string(),
        content_type: kind.default_content_type().map(str::to_string),
        byte_range,
        kind,
        depth: origin.depth,
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

fn render_playlist(
    entry: &ProxyEntry,
    parsed: &ParsedPlaylist,
    child_ids: &[String],
) -> Result<String, ProxyError> {
    let mut out = String::new();
    for line in &parsed.lines {
        match line {
            PlaylistLine::Text(text) => {
                push_rendered(&mut out, text)?;
                push_rendered(&mut out, "\n")?;
            }
            PlaylistLine::UriTag {
                text,
                uri_start,
                uri_end,
                spec,
            } => {
                if let Some(child_id) = child_ids.get(*spec) {
                    push_rendered(&mut out, &text[..*uri_start])?;
                    push_rendered(&mut out, &child_path(entry, child_id))?;
                    push_rendered(&mut out, &text[*uri_end..])?;
                } else {
                    push_rendered(&mut out, text)?;
                }
                push_rendered(&mut out, "\n")?;
            }
            PlaylistLine::Segment { spec } => {
                if let Some(child_id) = child_ids.get(*spec) {
                    push_rendered(&mut out, &child_path(entry, child_id))?;
                }
                push_rendered(&mut out, "\n")?;
            }
        }
    }
    Ok(out)
}

fn push_rendered(out: &mut String, value: &str) -> Result<(), ProxyError> {
    let Some(next) = out.len().checked_add(value.len()) else {
        return Err(ProxyError::Oversized("playlist-too-large"));
    };
    if next > MAX_RENDERED_PLAYLIST_BYTES {
        return Err(ProxyError::Oversized("playlist-too-large"));
    }
    out.push_str(value);
    Ok(())
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
    !crate::safe_http::is_public_ip(ip)
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
    use std::sync::atomic::{AtomicUsize, Ordering};
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

        fn requests(&self, needle: &str) -> usize {
            self.seen
                .lock()
                .unwrap()
                .iter()
                .filter(|(uri, _)| uri.contains(needle))
                .count()
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

    async fn deep_playlist(Path(n): Path<u8>) -> ([(header::HeaderName, &'static str); 1], String) {
        let body = if n < 6 {
            format!(
                "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=100000\n../{}/master.m3u8\n",
                n + 1
            )
        } else {
            "#EXTM3U\n#EXT-X-ENDLIST\n".to_string()
        };
        (
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
            body,
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
        const SHARED_RANGE_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
#EXT-X-BYTERANGE:500@0\n\
seg1.ts\n\
#EXT-X-BYTERANGE:500@500\n\
seg1.ts\n\
#EXT-X-ENDLIST\n";
        const MASTER_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-VERSION:7\n\
#EXT-X-DEFINE:NAME=\"dir\",VALUE=\"low\"\n\
#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"key.bin?auth=1\",IV=0x0123456789abcdef0123456789abcdef\n\
#EXT-X-SESSION-DATA:DATA-ID=\"com.example.title\",VALUE=\"Example\"\n\
#EXT-X-SESSION-DATA:DATA-ID=\"com.example.meta\",URI=\"meta.json\"\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"English\",URI=\"audio.m3u8?lang=en\"\n\
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"subs\",NAME=\"English\",URI=\"subs.vtt\"\n\
#EXT-X-STREAM-INF:BANDWIDTH=1280000,AUDIO=\"aud\",SUBTITLES=\"subs\"\n\
{$dir}.m3u8?auth=1\n\
#EXT-X-STREAM-INF:BANDWIDTH=2560000,AUDIO=\"aud\",SUBTITLES=\"subs\",URI=\"high.m3u8\"\n\
#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=100000,URI=\"iframe.m3u8\"\n";
        const HIGH_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
seg/seg2.ts\n\
#EXT-X-ENDLIST\n";
        const AUDIO_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
seg/seg2.ts\n\
#EXT-X-ENDLIST\n";
        const IFRAME_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
seg1.ts\n\
#EXT-X-ENDLIST\n";
        const IFRAME_MASTER_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=100000,URI=\"iframe.m3u8\"\n";
        const NESTED_BASE_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
#EXT-X-BYTERANGE:300@0\n\
seg1.ts\n\
#EXT-X-ENDLIST\n";
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
        let subtitle = || async {
            (
                [(header::CONTENT_TYPE, "text/vtt".to_string())],
                "WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nHello\n",
            )
        };
        let session_data = || async {
            (
                [(header::CONTENT_TYPE, "application/json".to_string())],
                r#"{"title":"Example"}"#,
            )
        };
        let redirect_nested = || async {
            (
                StatusCode::FOUND,
                [(header::LOCATION, "/hls/nested/base.m3u8".to_string())],
                Vec::<u8>::new(),
            )
        };
        let live_hits = Arc::new(AtomicUsize::new(0));
        let live = {
            let hits = live_hits.clone();
            move || {
                let hits = hits.clone();
                async move {
                    let window = hits.fetch_add(1, Ordering::SeqCst);
                    let body = format!(
                        "#EXTM3U\n\
#EXT-X-TARGETDURATION:4\n\
#EXT-X-MEDIA-SEQUENCE:{window}\n\
live/seg{}.ts\n\
live/seg{}.ts\n\
live/seg{}.ts\n",
                        window + 1,
                        window + 2,
                        window + 3
                    );
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
            .route(
                "/hls/shared.m3u8",
                test_get(playlist(SHARED_RANGE_PLAYLIST)),
            )
            .route("/hls/master.m3u8", test_get(playlist(MASTER_PLAYLIST)))
            .route("/hls/low.m3u8", test_get(playlist(MEDIA_PLAYLIST)))
            .route("/hls/high.m3u8", test_get(playlist(HIGH_PLAYLIST)))
            .route("/hls/audio.m3u8", test_get(playlist(AUDIO_PLAYLIST)))
            .route("/hls/iframe.m3u8", test_get(playlist(IFRAME_PLAYLIST)))
            .route(
                "/hls/iframe-master.m3u8",
                test_get(playlist(IFRAME_MASTER_PLAYLIST)),
            )
            .route("/hls/subs.vtt", test_get(subtitle))
            .route("/hls/meta.json", test_get(session_data))
            .route("/hls/redirect.m3u8", test_get(redirect_nested))
            .route(
                "/hls/nested/base.m3u8",
                test_get(playlist(NESTED_BASE_PLAYLIST)),
            )
            .route("/hls/deep/{n}/master.m3u8", test_get(deep_playlist))
            .route("/hls/live.m3u8", test_get(live))
            .route("/hls/large.m3u8", test_get(large_playlist))
            .route("/hls/bigkey.m3u8", test_get(playlist(BIG_KEY_PLAYLIST)))
            .route(
                "/hls/bigsubs.m3u8",
                test_get(playlist(BIG_SUBTITLE_PLAYLIST)),
            )
            .route("/hls/key.bin", test_get(key))
            .route("/hls/bigkey.bin", test_get(big_key))
            .route("/hls/bigsubs.vtt", test_get(big_subtitle))
            .route("/hls/seg1.ts", test_get(seg1.clone()).head(seg1.clone()))
            .route("/hls/nested/seg1.ts", test_get(seg1.clone()).head(seg1))
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

    struct CookieFixture {
        base: String,
        seen: SeenLog,
        _handle: tokio::task::JoinHandle<()>,
    }

    impl CookieFixture {
        fn cookie_values(&self, needle: &str) -> Vec<String> {
            self.seen
                .lock()
                .unwrap()
                .iter()
                .filter(|(uri, _)| uri.contains(needle))
                .flat_map(|(_, headers)| headers.get_all(header::COOKIE).iter())
                .filter_map(|value| value.to_str().ok().map(str::to_string))
                .collect()
        }
    }

    fn append_set_cookie(response: &mut Response, value: &str) {
        if let Ok(value) = HeaderValue::from_str(value) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }

    async fn cookie_playlist(Path(name): Path<String>) -> Response {
        let mut response = Response::new(Body::from(format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n/cookies/seg/{name}.ts\n#EXT-X-ENDLIST\n"
        )));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(PLAYLIST_CONTENT_TYPE),
        );
        match name.as_str() {
            "session" => append_set_cookie(&mut response, "sess=playlist; Path=/"),
            "expired" => {
                append_set_cookie(&mut response, "dead=1; Max-Age=0; Path=/");
                append_set_cookie(
                    &mut response,
                    "past=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/",
                );
                append_set_cookie(&mut response, "live=1; Max-Age=60; Path=/");
                append_set_cookie(
                    &mut response,
                    "mixed=1; Max-Age=60; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/",
                );
            }
            "secure" => {
                append_set_cookie(&mut response, "sec=1; Secure; Path=/");
                append_set_cookie(&mut response, "plain=1; Path=/");
            }
            "seed" => append_set_cookie(&mut response, "seed=server; Path=/"),
            "dup" => append_set_cookie(&mut response, "dup=server; Path=/"),
            "a" => append_set_cookie(&mut response, "a=1; Path=/"),
            "b" => append_set_cookie(&mut response, "b=1; Path=/"),
            "cap" => {
                for index in 0..=MAX_COOKIES {
                    append_set_cookie(&mut response, &format!("c{index}=1; Path=/"));
                }
            }
            "big" => {
                append_set_cookie(
                    &mut response,
                    &format!("big={}; Path=/", "x".repeat(MAX_COOKIE_BYTES + 1)),
                );
                append_set_cookie(&mut response, "small=1; Path=/");
            }
            _ => {}
        }
        response
    }

    async fn cookie_redirect() -> Response {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::FOUND;
        response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_static("/cookies/list/session/media.m3u8"),
        );
        append_set_cookie(&mut response, "redirect=1; Path=/");
        response
    }

    async fn cookie_segment() -> impl IntoResponse {
        ([(header::CONTENT_TYPE, "video/mp2t")], vec![7u8; 32])
    }

    async fn cookie_fixture() -> CookieFixture {
        let seen: SeenLog = Arc::new(StdMutex::new(Vec::new()));
        let router = Router::new()
            .route("/cookies/list/{name}/media.m3u8", test_get(cookie_playlist))
            .route("/cookies/redirect.m3u8", test_get(cookie_redirect))
            .route("/cookies/seg/{name}", test_get(cookie_segment))
            .layer(middleware::from_fn_with_state(seen.clone(), record_hls));
        let listener = TestListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        CookieFixture {
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

    async fn register_cookie_hls(
        store: &ProxyStore,
        fixture: &CookieFixture,
        name: &str,
        headers: Option<HashMap<String, String>>,
    ) -> ProxyUrl {
        store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/cookies/list/{name}/media.m3u8", fixture.base),
                "hls",
                headers,
            )
            .await
            .unwrap()
    }

    async fn fetch_cookie_manifest(
        client: &reqwest::Client,
        base: &str,
        registered: &ProxyUrl,
    ) -> String {
        let response = client
            .get(hls_proxy_url(base, registered))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.text().await.unwrap()
    }

    async fn fetch_cookie_segment(client: &reqwest::Client, base: &str, manifest: &str) {
        let segments = seg_paths(manifest);
        assert_eq!(segments.len(), 1, "{manifest}");
        let response = client
            .get(format!("{base}{}", segments[0]))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = response.bytes().await.unwrap();
    }

    #[tokio::test]
    async fn captures_playlist_cookie_and_sends_it_to_a_segment() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = register_cookie_hls(&store, &fixture, "session", None).await;
        let client = reqwest::Client::new();

        let manifest = fetch_cookie_manifest(&client, &base, &registered).await;
        fetch_cookie_segment(&client, &base, &manifest).await;

        assert_eq!(
            fixture.cookie_values("/cookies/seg/session.ts"),
            vec!["sess=playlist".to_string()]
        );
    }

    #[tokio::test]
    async fn captures_cookie_from_a_redirect() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/cookies/redirect.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let manifest = fetch_cookie_manifest(&client, &base, &registered).await;
        fetch_cookie_segment(&client, &base, &manifest).await;

        assert_eq!(
            fixture.cookie_values("/cookies/seg/session.ts"),
            vec!["redirect=1; sess=playlist".to_string()]
        );
    }

    #[tokio::test]
    async fn expired_cookies_are_dropped_and_max_age_wins() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = register_cookie_hls(&store, &fixture, "expired", None).await;
        let client = reqwest::Client::new();

        let manifest = fetch_cookie_manifest(&client, &base, &registered).await;
        fetch_cookie_segment(&client, &base, &manifest).await;

        let values = fixture.cookie_values("/cookies/seg/expired.ts");
        assert_eq!(values.len(), 1, "{values:?}");
        let cookie = &values[0];
        assert!(cookie.contains("live=1"), "{cookie}");
        assert!(cookie.contains("mixed=1"), "{cookie}");
        assert!(!cookie.contains("dead=1"), "{cookie}");
        assert!(!cookie.contains("past=1"), "{cookie}");
    }

    #[tokio::test]
    async fn secure_cookie_is_not_sent_over_http() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = register_cookie_hls(&store, &fixture, "secure", None).await;
        let client = reqwest::Client::new();

        let manifest = fetch_cookie_manifest(&client, &base, &registered).await;
        fetch_cookie_segment(&client, &base, &manifest).await;

        let values = fixture.cookie_values("/cookies/seg/secure.ts");
        assert_eq!(values, vec!["plain=1".to_string()], "{values:?}");
    }

    #[tokio::test]
    async fn cookie_jars_do_not_leak_across_entries() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let client = reqwest::Client::new();
        let first = register_cookie_hls(&store, &fixture, "a", None).await;
        let second = register_cookie_hls(&store, &fixture, "b", None).await;

        let first_manifest = fetch_cookie_manifest(&client, &base, &first).await;
        let second_manifest = fetch_cookie_manifest(&client, &base, &second).await;
        fetch_cookie_segment(&client, &base, &first_manifest).await;
        fetch_cookie_segment(&client, &base, &second_manifest).await;

        assert_eq!(
            fixture.cookie_values("/cookies/seg/a.ts"),
            vec!["a=1".to_string()]
        );
        assert_eq!(
            fixture.cookie_values("/cookies/seg/b.ts"),
            vec!["b=1".to_string()]
        );
    }

    #[tokio::test]
    async fn extension_cookie_seeds_once_and_a_captured_value_replaces_it() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let seed = HashMap::from([("Cookie".to_string(), "seed=ext; other=ext2".to_string())]);
        let registered = register_cookie_hls(&store, &fixture, "seed", Some(seed)).await;
        let client = reqwest::Client::new();

        let manifest = fetch_cookie_manifest(&client, &base, &registered).await;
        assert_eq!(
            fixture.cookie_values("/cookies/list/seed/media.m3u8"),
            vec!["seed=ext; other=ext2".to_string()]
        );

        fetch_cookie_segment(&client, &base, &manifest).await;
        let values = fixture.cookie_values("/cookies/seg/seed.ts");
        assert_eq!(values, vec!["seed=server; other=ext2".to_string()]);
        assert_eq!(values[0].matches("seed=").count(), 1, "{values:?}");
    }

    #[tokio::test]
    async fn duplicate_cookie_names_are_sent_once() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let seed = HashMap::from([("Cookie".to_string(), "dup=ext; dup=ext2".to_string())]);
        let registered = register_cookie_hls(&store, &fixture, "dup", Some(seed)).await;
        let client = reqwest::Client::new();

        let manifest = fetch_cookie_manifest(&client, &base, &registered).await;
        assert_eq!(
            fixture.cookie_values("/cookies/list/dup/media.m3u8"),
            vec!["dup=ext".to_string()]
        );

        fetch_cookie_segment(&client, &base, &manifest).await;
        let values = fixture.cookie_values("/cookies/seg/dup.ts");
        assert_eq!(values, vec!["dup=server".to_string()]);
        assert_eq!(values[0].matches("dup=").count(), 1, "{values:?}");
    }

    #[tokio::test]
    async fn revoke_drops_the_entry_and_its_cookie_jar() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = register_cookie_hls(&store, &fixture, "session", None).await;
        let client = reqwest::Client::new();

        let manifest = fetch_cookie_manifest(&client, &base, &registered).await;
        fetch_cookie_segment(&client, &base, &manifest).await;
        assert_eq!(store.status().entries, 1);

        store.revoke(&registered.opaque_id).unwrap();
        assert_eq!(store.status().entries, 0);

        let response = client
            .get(format!("{base}{}", seg_paths(&manifest)[0]))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let fresh = register_cookie_hls(&store, &fixture, "none", None).await;
        let fresh_manifest = fetch_cookie_manifest(&client, &base, &fresh).await;
        assert!(!fresh_manifest.is_empty());
        assert!(
            fixture
                .cookie_values("/cookies/list/none/media.m3u8")
                .is_empty(),
            "the old jar survived revoke"
        );
    }

    #[tokio::test]
    async fn cookie_jar_caps_count_and_per_cookie_size() {
        let fixture = cookie_fixture().await;
        let (store, base, _handle) = proxy().await;
        let client = reqwest::Client::new();

        let registered = register_cookie_hls(&store, &fixture, "cap", None).await;
        let manifest = fetch_cookie_manifest(&client, &base, &registered).await;
        assert!(!manifest.is_empty());
        {
            let state = store.inner.lock().unwrap();
            let entry = state.entries.get(&registered.opaque_id).unwrap();
            let jar = entry.jar.lock().unwrap();
            assert_eq!(jar.cookies.len(), MAX_COOKIES);
            assert!(!jar.cookies.iter().any(|cookie| cookie.name == "c0"));
            assert!(jar.cookies.iter().any(|cookie| cookie.name == "c32"));
        }

        let big = register_cookie_hls(&store, &fixture, "big", None).await;
        let manifest = fetch_cookie_manifest(&client, &base, &big).await;
        assert!(!manifest.is_empty());
        {
            let state = store.inner.lock().unwrap();
            let entry = state.entries.get(&big.opaque_id).unwrap();
            let jar = entry.jar.lock().unwrap();
            assert_eq!(jar.cookies.len(), 1);
            assert_eq!(jar.cookies[0].name, "small");
        }
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
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 0-999/2000"
        );
        assert_eq!(response.bytes().await.unwrap().len(), 1000);
        let seen = fixture.seen_for("/hls/seg1.ts").unwrap();
        assert_eq!(seen.get("referer").unwrap(), "https://source.example/");
        assert_eq!(seen.get("cookie").unwrap(), "sid=abc");
        assert_eq!(seen.get(header::RANGE).unwrap(), "bytes=0-999");

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
    async fn hls_same_url_keeps_distinct_byte_range_children() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/shared.m3u8", fixture.base),
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
        assert_ne!(segments[0], segments[1], "{manifest}");

        {
            let state = store.inner.lock().unwrap();
            let entry = state.entries.get(&registered.opaque_id).unwrap();
            let children = entry.children.lock().unwrap();
            assert_eq!(children.by_id.len(), 2);
            let ranges = children
                .by_id
                .values()
                .map(|child| child.byte_range.clone())
                .collect::<Vec<_>>();
            assert!(ranges.contains(&Some("0-499".to_string())), "{ranges:?}");
            assert!(ranges.contains(&Some("500-999".to_string())), "{ranges:?}");
        }

        let response = client
            .get(format!("{base}{}", segments[0]))
            .header(header::RANGE, "bytes=1500-1599")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 0-499/2000"
        );
        assert_eq!(response.bytes().await.unwrap().len(), 500);
        let seen = fixture.seen_for("/hls/seg1.ts").unwrap();
        assert_eq!(seen.get(header::RANGE).unwrap(), "bytes=0-499");

        let response = client
            .get(format!("{base}{}", segments[1]))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 500-999/2000"
        );
        assert_eq!(response.bytes().await.unwrap().len(), 500);
        let seen = fixture.seen_for("/hls/seg1.ts").unwrap();
        assert_eq!(seen.get(header::RANGE).unwrap(), "bytes=500-999");
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
            .lock()
            .unwrap()
            .by_id
            .len();
        assert_eq!(children, 4);
    }

    fn child_id_from_path(path: &str) -> String {
        path.split('/')
            .nth(4)
            .and_then(|rest| rest.split('?').next())
            .unwrap_or_default()
            .to_string()
    }

    #[tokio::test]
    async fn live_playlist_refreshes_keep_overlapping_child_ids() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/live.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let mut windows = Vec::new();
        for _ in 0..3 {
            let response = client
                .get(hls_proxy_url(&base, &registered))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                "no-store"
            );
            let text = response.text().await.unwrap();
            assert!(!text.contains("#EXT-X-ENDLIST"), "{text}");
            let ids = seg_paths(&text)
                .iter()
                .map(|path| child_id_from_path(path))
                .collect::<Vec<_>>();
            assert_eq!(ids.len(), 3, "{text}");
            windows.push(ids);
        }

        assert_eq!(fixture.requests("/hls/live.m3u8"), 3, "playlist was cached");
        assert_eq!(
            windows[0][1], windows[1][0],
            "overlapping segment changed id"
        );
        assert_eq!(
            windows[0][2], windows[1][1],
            "overlapping segment changed id"
        );
        assert_eq!(
            windows[1][1], windows[2][0],
            "overlapping segment changed id"
        );
        assert_eq!(
            windows[1][2], windows[2][1],
            "overlapping segment changed id"
        );
        assert_ne!(windows[0][0], windows[1][2], "new segment reused an id");
        assert_ne!(windows[1][0], windows[2][2], "new segment reused an id");
        assert!(
            !windows[0].contains(&windows[2][2]),
            "new segment reused an id"
        );

        let children = store
            .inner
            .lock()
            .unwrap()
            .entries
            .get(&registered.opaque_id)
            .unwrap()
            .children
            .lock()
            .unwrap()
            .by_id
            .len();
        assert_eq!(children, 5, "live child registry grew beyond unique URLs");
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
            .header(header::RANGE, "bytes=10-19")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 10-19/2000"
        );
        assert_eq!(response.bytes().await.unwrap().len(), 10);
        let seen = fixture.seen_for("/hls/seg1.ts").unwrap();
        assert_eq!(seen.get(header::RANGE).unwrap(), "bytes=10-19");
    }

    #[tokio::test]
    async fn hls_master_playlist_rewrites_variants_audio_and_subtitles() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/master.m3u8", fixture.base),
                "hls",
                Some(headers()),
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
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            PLAYLIST_CONTENT_TYPE
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );
        let text = response.text().await.unwrap();

        assert!(!text.contains(&fixture.base), "{text}");
        assert!(!text.contains("https://"), "{text}");
        for name in ["low.m3u8", "high.m3u8", "audio.m3u8", "subs.vtt"] {
            assert!(!text.contains(name), "{name}: {text}");
        }
        assert!(
            text.contains("#EXT-X-STREAM-INF:BANDWIDTH=1280000"),
            "{text}"
        );
        assert!(
            text.contains("#EXT-X-STREAM-INF:BANDWIDTH=2560000"),
            "{text}"
        );
        assert!(text.contains("#EXT-X-MEDIA:TYPE=AUDIO"), "{text}");
        assert!(text.contains("#EXT-X-MEDIA:TYPE=SUBTITLES"), "{text}");
        assert_eq!(text.matches("/s/").count(), 7, "{text}");

        let low_path = seg_paths(&text).first().unwrap().clone();
        let low = client
            .get(format!("{base}{low_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(low.status(), StatusCode::OK);
        let low_text = low.text().await.unwrap();
        assert!(low_text.contains("#EXT-X-KEY"), "{low_text}");
        assert!(!low_text.contains("seg1.ts"), "{low_text}");
        assert!(
            fixture.seen_for("/hls/low.m3u8?auth=1").is_some(),
            "the variant query was not preserved"
        );

        let audio_line = text
            .lines()
            .find(|line| line.starts_with("#EXT-X-MEDIA:TYPE=AUDIO"))
            .unwrap();
        let audio_path = child_path_from_line(audio_line).unwrap();
        let audio = client
            .get(format!("{base}{audio_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(audio.status(), StatusCode::OK);
        assert!(audio.text().await.unwrap().contains("/seg/"));
        assert!(fixture.seen_for("/hls/audio.m3u8?lang=en").is_some());

        let subtitle_line = text
            .lines()
            .find(|line| line.starts_with("#EXT-X-MEDIA:TYPE=SUBTITLES"))
            .unwrap();
        let subtitle_path = child_path_from_line(subtitle_line).unwrap();
        let subtitle = client
            .get(format!("{base}{subtitle_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(subtitle.status(), StatusCode::OK);
        assert!(subtitle.text().await.unwrap().contains("WEBVTT"));
        assert!(fixture.seen_for("/hls/subs.vtt").is_some());
    }

    #[tokio::test]
    async fn hls_variant_request_is_rewritten_as_a_media_playlist() {
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
        let client = reqwest::Client::new();

        let master = client
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let high_line = master
            .lines()
            .find(|line| line.starts_with("#EXT-X-STREAM-INF") && line.contains("URI="))
            .unwrap();
        let high_path = child_path_from_line(high_line).unwrap();

        let variant = client
            .get(format!("{base}{high_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(variant.status(), StatusCode::OK);
        assert_eq!(
            variant.headers().get(header::CONTENT_TYPE).unwrap(),
            PLAYLIST_CONTENT_TYPE
        );
        let variant_text = variant.text().await.unwrap();
        assert!(!variant_text.contains("seg/seg2.ts"), "{variant_text}");
        assert!(!variant_text.contains("https://"), "{variant_text}");
        let segments = seg_paths(&variant_text);
        assert_eq!(segments.len(), 1, "{variant_text}");

        let segment = client
            .get(format!("{base}{}", segments[0]))
            .send()
            .await
            .unwrap();
        assert_eq!(segment.status(), StatusCode::OK);
        assert_eq!(segment.bytes().await.unwrap().len(), 2000);
        assert!(fixture.seen_for("/hls/seg/seg2.ts").is_some());
    }

    #[tokio::test]
    async fn hls_iframe_variant_is_rewritten() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/iframe-master.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let master = client
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let iframe_line = master
            .lines()
            .find(|line| line.starts_with("#EXT-X-I-FRAME-STREAM-INF"))
            .unwrap();
        assert!(iframe_line.contains("/s/"), "{iframe_line}");
        let iframe_path = child_path_from_line(iframe_line).unwrap();

        let iframe = client
            .get(format!("{base}{iframe_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(iframe.status(), StatusCode::OK);
        let iframe_text = iframe.text().await.unwrap();
        assert_eq!(seg_paths(&iframe_text).len(), 1, "{iframe_text}");
        assert!(fixture.seen_for("/hls/iframe.m3u8").is_some());
    }

    #[tokio::test]
    async fn hls_session_key_and_data_are_rewritten() {
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
        let client = reqwest::Client::new();

        let master = client
            .get(hls_proxy_url(&base, &registered))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        let key_line = master
            .lines()
            .find(|line| line.starts_with("#EXT-X-SESSION-KEY"))
            .unwrap();
        assert!(key_line.contains("/s/"), "{key_line}");
        assert!(
            key_line.contains("IV=0x0123456789abcdef0123456789abcdef"),
            "{key_line}"
        );
        let key_path = child_path_from_line(key_line).unwrap();
        let key = client
            .get(format!("{base}{key_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(key.status(), StatusCode::OK);
        assert_eq!(key.bytes().await.unwrap().as_ref(), &[0x42u8; 16]);
        assert!(fixture.seen_for("/hls/key.bin?auth=1").is_some());

        let value_line = master
            .lines()
            .find(|line| line.contains("DATA-ID=\"com.example.title\""))
            .unwrap();
        assert!(
            value_line.starts_with("#EXT-X-SESSION-DATA") && !value_line.contains("URI="),
            "{value_line}"
        );

        let data_line = master
            .lines()
            .find(|line| line.contains("DATA-ID=\"com.example.meta\""))
            .unwrap();
        assert!(data_line.contains("/s/"), "{data_line}");
        let data_path = child_path_from_line(data_line).unwrap();
        let data = client
            .get(format!("{base}{data_path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(data.status(), StatusCode::OK);
        assert!(data.text().await.unwrap().contains("Example"));
        assert!(fixture.seen_for("/hls/meta.json").is_some());
    }

    #[tokio::test]
    async fn hls_nested_master_depth_is_capped() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/deep/0/master.m3u8", fixture.base),
                "hls",
                None,
            )
            .await
            .unwrap();
        let client = reqwest::Client::new();

        let mut path = hls_proxy_url(&base, &registered);
        for depth in 0..=MAX_PLAYLIST_DEPTH {
            let response = client.get(path).send().await.unwrap();
            let status = response.status();
            let text = response.text().await.unwrap();
            assert_eq!(status, StatusCode::OK, "depth {depth}: {text}");
            let next = seg_paths(&text);
            assert_eq!(next.len(), 1, "depth {depth}: {text}");
            path = format!("{base}{}", next[0]);
        }

        let response = client.get(path).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.text().await.unwrap(), "playlist-depth");
    }

    #[tokio::test]
    async fn hls_redirect_resolves_children_against_the_final_url() {
        let fixture = hls_fixture().await;
        let (store, base, _handle) = proxy().await;
        let registered = store
            .register_fixture_with_kind(
                "app.kaminari.test",
                &format!("{}/hls/redirect.m3u8", fixture.base),
                "hls",
                None,
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
        assert!(manifest.contains("#EXT-X-BYTERANGE:300@0"), "{manifest}");
        let segments = seg_paths(&manifest);
        assert_eq!(segments.len(), 1, "{manifest}");

        let response = client
            .get(format!("{base}{}", segments[0]))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.bytes().await.unwrap().len(), 300);
        assert!(
            fixture.seen_for("/hls/nested/seg1.ts").is_some(),
            "the child did not resolve against the redirect target"
        );
        assert!(
            fixture.seen_for("/hls/seg1.ts").is_none(),
            "the child resolved against the pre redirect URL"
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
    async fn long_vod_playlist_keeps_its_first_segments() {
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

        // A 23 minute episode at roughly five seconds per segment is about
        // 283 children. The old 256 child cap evicted the first segments and
        // hls.js stopped at the first missing child.
        let specs = (0..283)
            .map(|index| ChildSpec {
                upstream_url: format!("http://127.0.0.1:9/seg/{index}.ts"),
                content_type: None,
                byte_range: None,
                kind: ChildKind::Segment,
                depth: 1,
            })
            .collect::<Vec<_>>();
        let ids = store
            .reconcile_children(&registered.opaque_id, &specs)
            .unwrap();
        assert_eq!(ids.len(), 283);
        let state = store.inner.lock().unwrap();
        let entry = state.entries.get(&registered.opaque_id).unwrap();
        let children = entry.children.lock().unwrap();
        assert_eq!(children.by_id.len(), 283);
        for id in &ids {
            assert!(children.by_id.contains_key(id), "missing early child {id}");
        }
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
                depth: 1,
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
                .lock()
                .unwrap()
                .by_id
                .len(),
            10
        );

        let duplicate = ChildSpec {
            upstream_url: "http://127.0.0.1:9/seg/dup.ts".to_string(),
            content_type: None,
            byte_range: None,
            kind: ChildKind::Segment,
            depth: 1,
        };
        let ids = store
            .reconcile_children(&registered.opaque_id, &[duplicate.clone(), duplicate])
            .unwrap();
        assert_eq!(ids[0], ids[1]);

        // A single refresh cannot exceed the entry budget. It fails before
        // mutating the table, so previously issued child URLs stay valid.
        let many = (0..MAX_CHILDREN + 40)
            .map(|index| ChildSpec {
                upstream_url: format!("http://127.0.0.1:9/many/{index}.ts"),
                content_type: None,
                byte_range: None,
                kind: ChildKind::Segment,
                depth: 1,
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            store.reconcile_children(&registered.opaque_id, &many),
            Err(ProxyError::Oversized("playlist-too-many-children"))
        ));
        assert_eq!(
            store
                .inner
                .lock()
                .unwrap()
                .entries
                .get(&registered.opaque_id)
                .unwrap()
                .children
                .lock()
                .unwrap()
                .by_id
                .len(),
            11
        );

        // Fill the table with children that are not in the next playlist.
        // The next refresh must evict those first and keep every child it
        // references, even at the cap.
        let stale = (0..40)
            .map(|index| ChildSpec {
                upstream_url: format!("http://127.0.0.1:9/stale/{index}.ts"),
                content_type: None,
                byte_range: None,
                kind: ChildKind::Segment,
                depth: 1,
            })
            .collect::<Vec<_>>();
        store
            .reconcile_children(&registered.opaque_id, &stale)
            .unwrap();

        let mut refresh = specs.clone();
        refresh.push(ChildSpec {
            upstream_url: "http://127.0.0.1:9/seg/dup.ts".to_string(),
            content_type: None,
            byte_range: None,
            kind: ChildKind::Segment,
            depth: 1,
        });
        refresh.extend((0..MAX_CHILDREN - refresh.len()).map(|index| ChildSpec {
            upstream_url: format!("http://127.0.0.1:9/refresh/{index}.ts"),
            content_type: None,
            byte_range: None,
            kind: ChildKind::Segment,
            depth: 1,
        }));
        let refresh_ids = store
            .reconcile_children(&registered.opaque_id, &refresh)
            .unwrap();
        assert_eq!(refresh_ids.len(), MAX_CHILDREN);
        assert_eq!(&refresh_ids[..10], &first[..]);
        assert_eq!(refresh_ids[10], ids[0]);
        let state = store.inner.lock().unwrap();
        let entry = state.entries.get(&registered.opaque_id).unwrap();
        let children = entry.children.lock().unwrap();
        assert_eq!(children.by_id.len(), MAX_CHILDREN);
        for id in &refresh_ids {
            assert!(
                children.by_id.contains_key(id),
                "evicted current child {id}"
            );
        }
    }

    #[test]
    fn resolves_relative_playlist_urls_and_hides_the_origin() {
        let base = Url::parse("https://cdn.example/hls/media.m3u8").unwrap();
        let text = "#EXTM3U\n\
#EXT-X-DEFINE:NAME=\"dir\",VALUE=\"seg\"\n\
#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0xABCD\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"en\",URI=\"audio.m3u8\"\n\
{$dir}/1.ts?token=abc\n";
        let parsed = parse_playlist(&base, text, 0).unwrap();
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
        let rendered = render_playlist(&entry, &parsed, &child_ids).unwrap();
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

    #[test]
    fn rendered_playlist_bytes_are_capped() {
        let entry = test_entry();
        let parsed = ParsedPlaylist {
            lines: vec![PlaylistLine::Text(
                "x".repeat(MAX_RENDERED_PLAYLIST_BYTES + 1),
            )],
            specs: Vec::new(),
        };
        assert!(matches!(
            render_playlist(&entry, &parsed, &[]),
            Err(ProxyError::Oversized("playlist-too-large"))
        ));
    }

    #[test]
    fn cross_origin_children_must_be_in_the_manifest_allowlist() {
        let base = Url::parse("https://origin.example/hls/master.m3u8").unwrap();
        let text = "#EXTM3U\n\
#EXT-X-STREAM-INF:BANDWIDTH=1\n\
https://cdn.example/hls/low.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=2\n\
https://other.example/hls/high.m3u8\n";
        let parsed = parse_playlist(&base, text, 0).unwrap();
        assert_eq!(parsed.specs.len(), 2);
        assert_eq!(parsed.specs[0].depth, 1);
        assert_eq!(parsed.specs[1].depth, 1);

        let mut entry = test_entry();
        entry.allowed = Some(vec![HostRule::parse("https://cdn.example/*").unwrap()]);
        assert!(check_specs_allowed(&entry, &parsed.specs[..1]).is_ok());
        assert!(matches!(
            check_specs_allowed(&entry, &parsed.specs[1..]),
            Err(ProxyError::ChildNotAllowed)
        ));
        assert!(matches!(
            check_specs_allowed(&entry, &parsed.specs),
            Err(ProxyError::ChildNotAllowed)
        ));

        entry.allowed = Some(Vec::new());
        assert!(matches!(
            check_specs_allowed(&entry, &parsed.specs[..1]),
            Err(ProxyError::ChildNotAllowed)
        ));
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
            jar: Arc::new(Mutex::new(CookieJar::default())),
            allowed: None,
            allow_private: true,
            created_at: now,
            last_used: Mutex::new(now),
            children: Mutex::new(ChildIndex::new()),
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
    fn cookie_domains_use_the_embedded_dot_rule() {
        let url = Url::parse("https://cdn.example.com/hls/media.m3u8").unwrap();
        let now = SystemTime::now();
        assert!(parse_set_cookie("a=1; Domain=example.com; Path=/", &url, now).is_some());
        assert!(parse_set_cookie("a=1; Domain=.example.com; Path=/", &url, now).is_some());
        assert!(parse_set_cookie("a=1; Domain=com; Path=/", &url, now).is_none());
        assert!(parse_set_cookie("a=1; Domain=ample.com; Path=/", &url, now).is_none());
        assert!(parse_set_cookie("a=1; Domain=other.com; Path=/", &url, now).is_none());
        assert!(parse_set_cookie("a=1; SameSite=None; Path=/", &url, now).is_none());
        assert!(parse_set_cookie("a=1; SameSite=None; Secure; Path=/", &url, now).is_some());
    }

    #[test]
    fn cookie_jar_is_shared_by_entry_handles() {
        let entry = Arc::new(test_entry());
        let clone = Arc::clone(&entry);
        let url = Url::parse("https://cdn.example/hls/media.m3u8").unwrap();
        let headers = HeaderMap::from_iter([(
            header::SET_COOKIE,
            HeaderValue::from_static("shared=1; Path=/"),
        )]);
        entry.capture_cookies(&headers, &url);
        assert_eq!(
            clone.cookie_header(&url),
            Some("shared=1".to_string()),
            "a clone kept a separate jar"
        );
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

    #[tokio::test]
    async fn download_authority_accepts_only_issued_proxy_urls() {
        let store = ProxyStore::for_tests(8765);
        let stream = ProxyStreamInput {
            url: "http://127.0.0.1:9/video.mp4".to_string(),
            kind: "file".to_string(),
            headers: None,
        };
        let registered = store
            .register_inner("app.kaminari.test", &stream, None, false)
            .await
            .unwrap();

        assert!(store.authorizes_url(&registered.proxy_url));
        let child = registered.proxy_url.replace("/file", "/seg/segment-1");
        assert!(store.authorizes_url(&child));

        let wrong_token = format!("{}0", registered.proxy_url);
        assert!(!store.authorizes_url(&wrong_token));
        let wrong_port = registered.proxy_url.replace(":8765", ":8766");
        assert!(!store.authorizes_url(&wrong_port));
        let wrong_host = registered.proxy_url.replace("127.0.0.1", "localhost");
        assert!(!store.authorizes_url(&wrong_host));
        let no_token = registered.proxy_url.split('?').next().unwrap();
        assert!(!store.authorizes_url(no_token));

        store.revoke(&registered.opaque_id).unwrap();
        assert!(!store.authorizes_url(&registered.proxy_url));
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
