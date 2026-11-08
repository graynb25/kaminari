// Loopback stream proxy for protected MP4 playback. See docs/PLAN-STREAM-PROXY.md.
// The WebView gets an opaque loopback URL. Rust keeps the upstream URL, the
// allowlisted headers and the per entry token in memory only.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::TryStreamExt;
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
pub const MAX_RANGE_SKIP: u64 = 16 * 1024 * 1024;
pub const STREAM_CHUNK_BYTES: usize = 64 * 1024;
pub const SWEEP_SECS: u64 = 60;

const MAX_REDIRECTS: usize = 3;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const DNS_TIMEOUT: Duration = Duration::from_secs(10);
const EXPIRES_IN_SEC: u64 = 2 * 60 * 60;

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

enum ProxyError {
    Upstream(&'static str),
    Timeout,
}

struct ByteRange {
    start: u64,
    end: Option<u64>,
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
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
            .route("/healthz", get(healthz))
            .route("/s/{id}/file", get(file_handler).head(file_handler))
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
        let entry = ProxyEntry {
            opaque_id: opaque_id.clone(),
            token: token.clone(),
            extension_id: extension_id.to_string(),
            upstream_url: url.to_string(),
            kind: stream.kind.to_ascii_lowercase(),
            headers,
            allowed,
            allow_private,
            created_at: now,
            last_used: now,
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

        Ok(ProxyUrl {
            proxy_url: format!("http://127.0.0.1:{port}/s/{opaque_id}/file?token={token}"),
            opaque_id,
            expires_in_sec: EXPIRES_IN_SEC,
            kind: stream.kind.to_ascii_lowercase(),
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
        if !constant_time_eq(entry.token.as_bytes(), token.as_bytes()) {
            return Err(LookupError::Forbidden);
        }
        let now = Instant::now();
        if now.duration_since(entry.created_at) >= ABSOLUTE_TTL
            || now.duration_since(entry.last_used) >= IDLE_TTL
        {
            return Err(LookupError::Expired);
        }
        let entry = state.entries.get_mut(id).ok_or(LookupError::NotFound)?;
        entry.last_used = now;
        Ok(entry.clone())
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
        let stream = ProxyStreamInput {
            url: url.to_string(),
            kind: "mp4".to_string(),
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

async fn healthz(State(store): State<ProxyStore>) -> Response {
    let status = store.status();
    axum::Json(serde_json::json!({
        "ok": status.enabled,
        "port": status.port,
    }))
    .into_response()
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
    let original = Url::parse(&entry.upstream_url).map_err(|_| ProxyError::Upstream("url"))?;
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
            return Ok(response);
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
        ProxyError::Upstream(reason) => {
            tracing::debug!(reason, "proxy: upstream request failed");
            short(StatusCode::BAD_GATEWAY, "upstream-error")
        }
    }
}

fn short(status: StatusCode, body: &'static str) -> Response {
    (status, body).into_response()
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
    let mut bytes = [0u8; 16];
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

    fn proxy_url(base: &str, registered: &ProxyUrl) -> String {
        format!(
            "{base}/s/{}/file?token={}",
            registered.opaque_id,
            token_of(registered)
        )
    }
}
