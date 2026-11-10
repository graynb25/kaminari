// Stream downloads: direct media and simple HLS playlists.
// Rust owns the network and disk. Progress reaches the UI by event.
// Headers and tokens never reach logs or error text.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use url::Origin;

use crate::proxy::ProxyStore;
use crate::safe_http::{self, SafeFetch};

const DOWNLOAD_EVENT: &str = "download-progress";
const MAX_HEADERS: usize = 16;
const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_PLAYLIST_BYTES: usize = 2 * 1024 * 1024;
const MAX_HLS_SEGMENTS: usize = 10_000;
const MAX_HLS_DEPTH: u8 = 3;
const MAX_FILE_NAME_CHARS: usize = 140;
const MAX_COLLISION_TRIES: usize = 1000;
const MAX_DOWNLOADS: usize = 100;
const MAX_DOWNLOAD_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const HEADER_TIMEOUT: Duration = Duration::from_secs(30);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

const ALLOWED_HEADERS: [&str; 7] = [
    "user-agent",
    "referer",
    "origin",
    "cookie",
    "authorization",
    "accept",
    "accept-language",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub url: String,
    #[serde(default)]
    pub proxied_url: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub kind: Option<String>,
    pub folder: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Running,
    Done,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub received: u64,
    pub total: Option<u64>,
    pub state: DownloadStatus,
    pub path: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadStarted {
    pub id: String,
}

#[derive(Clone)]
struct CancelToken {
    sender: Arc<watch::Sender<bool>>,
}

impl Default for CancelToken {
    fn default() -> Self {
        let (sender, _) = watch::channel(false);
        Self {
            sender: Arc::new(sender),
        }
    }
}

impl CancelToken {
    fn cancel(&self) {
        self.sender.send_replace(true);
    }

    fn is_cancelled(&self) -> bool {
        *self.sender.borrow()
    }

    async fn cancelled(&self) {
        let mut receiver = self.sender.subscribe();
        if *receiver.borrow() {
            return;
        }
        let _ = receiver.changed().await;
    }
}

struct DownloadRecord {
    cancel: CancelToken,
    progress: DownloadProgress,
}

#[derive(Clone, Default)]
pub struct DownloadState {
    inner: Arc<Mutex<HashMap<String, DownloadRecord>>>,
}

impl DownloadState {
    fn insert(
        &self,
        id: &str,
        cancel: CancelToken,
        progress: DownloadProgress,
    ) -> Result<(), String> {
        let mut map = self
            .inner
            .lock()
            .map_err(|_| "downloads are unavailable".to_string())?;
        if map.len() >= MAX_DOWNLOADS {
            let finished = map
                .iter()
                .find(|(_, record)| record.progress.state != DownloadStatus::Running)
                .map(|(key, _)| key.clone());
            match finished {
                Some(key) => {
                    map.remove(&key);
                }
                None => {
                    return Err(format!(
                        "Too many downloads are running. The limit is {MAX_DOWNLOADS}."
                    ));
                }
            }
        }
        map.insert(id.to_string(), DownloadRecord { cancel, progress });
        Ok(())
    }

    fn cancel(&self, id: &str) -> Result<(), String> {
        let map = self
            .inner
            .lock()
            .map_err(|_| "downloads are unavailable".to_string())?;
        let record = map
            .get(id)
            .ok_or_else(|| "download not found".to_string())?;
        record.cancel.cancel();
        Ok(())
    }

    fn current(&self, id: &str) -> Option<DownloadProgress> {
        let map = self.inner.lock().ok()?;
        map.get(id).map(|record| record.progress.clone())
    }

    fn update(&self, progress: &DownloadProgress) {
        if let Ok(mut map) = self.inner.lock() {
            if let Some(record) = map.get_mut(&progress.id) {
                record.progress = progress.clone();
            }
        }
    }

    fn snapshot(&self) -> Result<Vec<DownloadProgress>, String> {
        let map = self
            .inner
            .lock()
            .map_err(|_| "downloads are unavailable".to_string())?;
        let mut list = map
            .values()
            .map(|record| record.progress.clone())
            .collect::<Vec<_>>();
        list.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(list)
    }
}

#[tauri::command]
pub fn download_start(
    state: State<'_, DownloadState>,
    app: AppHandle,
    proxy: State<'_, ProxyStore>,
    request: DownloadRequest,
) -> Result<DownloadStarted, String> {
    let prepared = prepare(request, proxy.inner().clone())?;
    let id = new_download_id()?;
    let cancel = CancelToken::default();
    state.insert(
        &id,
        cancel.clone(),
        DownloadProgress {
            id: id.clone(),
            received: 0,
            total: None,
            state: DownloadStatus::Running,
            path: None,
            message: None,
        },
    )?;

    tracing::info!(id = %id, hls = prepared.is_hls, "download: started");
    let downloads = state.inner().clone();
    let task_id = id.clone();
    tauri::async_runtime::spawn(async move {
        run_download(app, downloads, task_id, prepared, cancel).await;
    });

    Ok(DownloadStarted { id })
}

#[tauri::command]
pub fn download_cancel(state: State<'_, DownloadState>, id: String) -> Result<(), String> {
    state.cancel(&id)
}

#[tauri::command]
pub fn downloads_list(state: State<'_, DownloadState>) -> Result<Vec<DownloadProgress>, String> {
    state.snapshot()
}

struct DownloadSource {
    start_url: String,
    origin: Origin,
    headers: Vec<(HeaderName, HeaderValue)>,
}

struct PreparedDownload {
    source: DownloadSource,
    is_hls: bool,
    kind: String,
    folder: PathBuf,
    file_name: String,
    direct_url: String,
    proxy: ProxyStore,
}

struct DownloadContext<'a> {
    app: &'a AppHandle,
    state: &'a DownloadState,
    id: &'a str,
    fetcher: &'a SafeFetch,
    source: &'a DownloadSource,
    proxy: &'a ProxyStore,
    cancel: &'a CancelToken,
}

#[derive(Debug)]
enum DownloadError {
    Cancelled,
    Message(String),
}

impl From<String> for DownloadError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

#[derive(Debug)]
struct HlsVariant {
    bandwidth: u64,
    url: String,
}

#[derive(Debug)]
struct HlsPlan {
    segments: Vec<String>,
    init: Option<String>,
    extension: &'static str,
}

struct ProgressSink<'a> {
    app: &'a AppHandle,
    state: &'a DownloadState,
    id: &'a str,
    last: Instant,
}

impl<'a> ProgressSink<'a> {
    fn new(app: &'a AppHandle, state: &'a DownloadState, id: &'a str) -> Self {
        Self {
            app,
            state,
            id,
            last: Instant::now() - PROGRESS_INTERVAL,
        }
    }

    fn running(&mut self, received: u64, total: Option<u64>) {
        if self.last.elapsed() < PROGRESS_INTERVAL {
            return;
        }
        self.last = Instant::now();
        self.publish(DownloadStatus::Running, received, total, None, None);
    }

    fn publish(
        &self,
        state: DownloadStatus,
        received: u64,
        total: Option<u64>,
        path: Option<String>,
        message: Option<String>,
    ) {
        emit_progress(
            self.app,
            self.state,
            DownloadProgress {
                id: self.id.to_string(),
                received,
                total,
                state,
                path,
                message,
            },
        );
    }
}

fn prepare(request: DownloadRequest, proxy: ProxyStore) -> Result<PreparedDownload, String> {
    let folder = validate_folder(&request.folder)?;
    let direct = validate_url(&request.url, &proxy)?;
    let proxied = request
        .proxied_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let effective = match proxied {
        Some(value) => validate_url(value, &proxy)?,
        None => direct.clone(),
    };
    let headers = if proxied.is_some() {
        Vec::new()
    } else {
        sanitize_headers(request.headers.as_ref())?
    };
    let kind = request.kind.unwrap_or_default().trim().to_ascii_lowercase();
    let is_hls =
        kind == "hls" || kind == "m3u8" || path_is_m3u8(&direct) || path_is_m3u8(&effective);
    let file_name = sanitize_file_name(&request.file_name)?;

    Ok(PreparedDownload {
        source: DownloadSource {
            start_url: effective.to_string(),
            origin: effective.origin(),
            headers,
        },
        is_hls,
        kind,
        folder,
        file_name,
        direct_url: request.url.trim().to_string(),
        proxy,
    })
}

fn validate_folder(folder: &str) -> Result<PathBuf, String> {
    let trimmed = folder.trim();
    if trimmed.is_empty() {
        return Err("Choose a downloads folder first.".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err("The downloads folder must be an absolute path.".to_string());
    }
    let metadata =
        std::fs::metadata(&path).map_err(|_| "The downloads folder does not exist.".to_string())?;
    if !metadata.is_dir() {
        return Err("The downloads folder is not a folder.".to_string());
    }
    Ok(path)
}

fn validate_url(value: &str, proxy: &ProxyStore) -> Result<Url, String> {
    let url = Url::parse(value.trim()).map_err(|_| "The stream URL is invalid.".to_string())?;
    if proxy.authorizes_url(url.as_str()) {
        return Ok(url);
    }
    safe_http::validate_https_url(&url)
        .map_err(|_| "The stream URL must be https and point to a public host.".to_string())?;
    Ok(url)
}

fn path_is_m3u8(url: &Url) -> bool {
    url.path().to_ascii_lowercase().ends_with(".m3u8")
}

fn sanitize_headers(
    input: Option<&HashMap<String, String>>,
) -> Result<Vec<(HeaderName, HeaderValue)>, String> {
    let Some(input) = input else {
        return Ok(Vec::new());
    };
    if input.len() > MAX_HEADERS {
        return Err(format!(
            "Too many stream headers. The limit is {MAX_HEADERS}."
        ));
    }

    let mut headers = Vec::new();
    let mut bytes = 0usize;
    for (name, value) in input {
        let lower = name.trim().to_ascii_lowercase();
        if lower.is_empty() || !ALLOWED_HEADERS.contains(&lower.as_str()) {
            continue;
        }
        if lower.contains(['\r', '\n', '\0']) || value.contains(['\r', '\n', '\0']) {
            return Err("Stream headers must not contain CR, LF or NUL.".to_string());
        }
        let header_name = HeaderName::from_bytes(lower.as_bytes())
            .map_err(|_| "A stream header name is invalid.".to_string())?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|_| "A stream header value is invalid.".to_string())?;
        bytes += lower.len() + value.len();
        if bytes > MAX_HEADER_BYTES {
            return Err("Stream headers are larger than 8 KB.".to_string());
        }
        headers.push((header_name, header_value));
    }
    Ok(headers)
}

async fn run_download(
    app: AppHandle,
    state: DownloadState,
    id: String,
    prepared: PreparedDownload,
    cancel: CancelToken,
) {
    let outcome = execute(&app, &state, &id, &prepared, &cancel).await;
    let current = state.current(&id);
    let (received, total) = current
        .as_ref()
        .map(|progress| (progress.received, progress.total))
        .unwrap_or((0, None));

    match outcome {
        Ok((path, bytes)) => {
            let path_text = path.to_string_lossy().to_string();
            let message = if prepared.is_hls {
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("ts");
                Some(format!(
                    "HLS segments were joined into a .{extension} file. Play the saved file in external mpv."
                ))
            } else {
                None
            };
            emit_progress(
                &app,
                &state,
                DownloadProgress {
                    id,
                    received: bytes,
                    total: Some(bytes),
                    state: DownloadStatus::Done,
                    path: Some(path_text),
                    message,
                },
            );
        }
        Err(DownloadError::Cancelled) => {
            emit_progress(
                &app,
                &state,
                DownloadProgress {
                    id,
                    received,
                    total,
                    state: DownloadStatus::Cancelled,
                    path: None,
                    message: Some("Download cancelled.".to_string()),
                },
            );
        }
        Err(DownloadError::Message(message)) => {
            emit_progress(
                &app,
                &state,
                DownloadProgress {
                    id,
                    received,
                    total,
                    state: DownloadStatus::Error,
                    path: None,
                    message: Some(message),
                },
            );
        }
    }
}

async fn execute(
    app: &AppHandle,
    state: &DownloadState,
    id: &str,
    prepared: &PreparedDownload,
    cancel: &CancelToken,
) -> Result<(PathBuf, u64), DownloadError> {
    let fetcher = build_client(&prepared.proxy)?;
    let context = DownloadContext {
        app,
        state,
        id,
        fetcher: &fetcher,
        source: &prepared.source,
        proxy: &prepared.proxy,
        cancel,
    };

    let mut plan: Option<HlsPlan> = None;
    let file_name = if prepared.is_hls {
        let resolved = resolve_hls(&context).await?;
        let name = hls_file_name(&prepared.file_name, resolved.extension)?;
        plan = Some(resolved);
        name
    } else {
        direct_file_name(&prepared.file_name, &prepared.direct_url, &prepared.kind)?
    };

    let (target, part) = unique_destination(&prepared.folder, &file_name)?;
    let result = if let Some(plan) = plan.as_ref() {
        perform_hls(&context, plan, &part).await
    } else {
        perform_direct(&context, &part).await
    };

    match result {
        Ok(0) => {
            let _ = tokio::fs::remove_file(&part).await;
            Err(DownloadError::Message(
                "The server returned an empty file.".to_string(),
            ))
        }
        Ok(received) => {
            if cancel.is_cancelled() {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(DownloadError::Cancelled);
            }
            if tokio::fs::rename(&part, &target).await.is_err() {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(DownloadError::Message(
                    "The finished file could not be moved into place.".to_string(),
                ));
            }
            Ok((target, received))
        }
        Err(error) => {
            let _ = tokio::fs::remove_file(&part).await;
            Err(error)
        }
    }
}

fn build_client(proxy: &ProxyStore) -> Result<SafeFetch, String> {
    let proxy = proxy.clone();
    SafeFetch::new_with_loopback_authorizer(
        "Kaminari",
        None,
        true, // stream bytes are written as received, never decompressed
        std::sync::Arc::new(move |url: &Url| proxy.authorizes_url(url.as_str())),
    )
    .map_err(|_| "The download client could not start.".to_string())
}

async fn perform_direct(context: &DownloadContext<'_>, part: &Path) -> Result<u64, DownloadError> {
    let response = send_get(context, &context.source.start_url).await?;
    let total = response.content_length();
    if total.is_some_and(|bytes| bytes > MAX_DOWNLOAD_BYTES) {
        return Err(DownloadError::Message(
            "The download is larger than the 20 GB limit.".to_string(),
        ));
    }
    let mut file = create_part_file(part).await?;
    let mut sink = ProgressSink::new(context.app, context.state, context.id);
    sink.publish(DownloadStatus::Running, 0, total, None, None);
    let received = write_response(&mut file, response, 0, total, context.cancel, &mut sink).await?;
    flush_file(&mut file).await?;
    Ok(received)
}

async fn perform_hls(
    context: &DownloadContext<'_>,
    plan: &HlsPlan,
    part: &Path,
) -> Result<u64, DownloadError> {
    let mut file = create_part_file(part).await?;
    let mut sink = ProgressSink::new(context.app, context.state, context.id);
    sink.publish(DownloadStatus::Running, 0, None, None, None);
    let mut received = 0u64;

    if let Some(init) = plan.init.as_ref() {
        let response = send_get(context, init).await?;
        received = write_response(
            &mut file,
            response,
            received,
            None,
            context.cancel,
            &mut sink,
        )
        .await?;
    }
    for segment in &plan.segments {
        if context.cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let response = send_get(context, segment).await?;
        received = write_response(
            &mut file,
            response,
            received,
            None,
            context.cancel,
            &mut sink,
        )
        .await?;
    }

    flush_file(&mut file).await?;
    Ok(received)
}

async fn create_part_file(path: &Path) -> Result<tokio::fs::File, DownloadError> {
    tokio::fs::File::create(path)
        .await
        .map_err(|_| DownloadError::Message("The download file could not be created.".to_string()))
}

async fn flush_file(file: &mut tokio::fs::File) -> Result<(), DownloadError> {
    file.flush()
        .await
        .map_err(|_| DownloadError::Message("The file could not be written.".to_string()))
}

async fn send_get(
    context: &DownloadContext<'_>,
    url: &str,
) -> Result<reqwest::Response, DownloadError> {
    if context.cancel.is_cancelled() {
        return Err(DownloadError::Cancelled);
    }
    let parsed = Url::parse(url)
        .map_err(|_| DownloadError::Message("The stream URL is invalid.".to_string()))?;

    let source_origin = context.source.origin.clone();
    let headers = &context.source.headers;
    let request = context
        .fetcher
        .get(parsed.as_str(), move |hop, mut request| {
            if hop.origin() == source_origin {
                for (name, value) in headers {
                    request = request.header(name, value);
                }
            }
            request
        });

    let response = tokio::select! {
        result = tokio::time::timeout(HEADER_TIMEOUT, request) => {
            result
                .map_err(|_| {
                    DownloadError::Message("The server took too long to answer.".to_string())
                })?
                .map_err(|error| DownloadError::Message(format!("The stream request failed: {error}")))?
        }
        _ = context.cancel.cancelled() => return Err(DownloadError::Cancelled),
    };

    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(DownloadError::Message(format!(
            "The server refused the download (HTTP {status})."
        )));
    }
    if context.cancel.is_cancelled() {
        return Err(DownloadError::Cancelled);
    }
    Ok(response)
}

async fn write_response(
    file: &mut tokio::fs::File,
    response: reqwest::Response,
    base: u64,
    total: Option<u64>,
    cancel: &CancelToken,
    sink: &mut ProgressSink<'_>,
) -> Result<u64, DownloadError> {
    let mut received = base;
    let mut stream = response.bytes_stream();
    loop {
        let next = tokio::select! {
            next = stream.next() => next,
            _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
        };
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(|error| DownloadError::Message(reqwest_message(&error)))?;
        if received.saturating_add(chunk.len() as u64) > MAX_DOWNLOAD_BYTES {
            return Err(DownloadError::Message(
                "The download is larger than the 20 GB limit.".to_string(),
            ));
        }
        file.write_all(&chunk)
            .await
            .map_err(|_| DownloadError::Message("The file could not be written.".to_string()))?;
        received += chunk.len() as u64;
        sink.running(received, total);
    }
    Ok(received)
}

fn reqwest_message(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "The server timed out.".to_string()
    } else if error.is_connect() {
        "The server could not be reached.".to_string()
    } else {
        "The download connection failed.".to_string()
    }
}

fn emit_progress(app: &AppHandle, state: &DownloadState, progress: DownloadProgress) {
    state.update(&progress);
    if let Err(error) = app.emit(DOWNLOAD_EVENT, &progress) {
        tracing::warn!(error = %error, "download: progress event failed");
    }
}

async fn resolve_hls(context: &DownloadContext<'_>) -> Result<HlsPlan, DownloadError> {
    let mut url = context.source.start_url.clone();
    for _ in 0..=MAX_HLS_DEPTH {
        let (text, final_url) = fetch_playlist(context, &url).await?;
        if is_master_playlist(&text) {
            validate_master_tags(&text)?;
            let variants = parse_master_playlist(&final_url, &text, context.proxy)?;
            let Some(best) = variants.into_iter().max_by_key(|variant| variant.bandwidth) else {
                return Err(DownloadError::Message(
                    "The HLS master playlist has no variants.".to_string(),
                ));
            };
            url = best.url;
            continue;
        }
        return Ok(parse_media_playlist(&final_url, &text, context.proxy)?);
    }
    Err(DownloadError::Message(
        "The HLS playlist nests too deeply.".to_string(),
    ))
}

async fn fetch_playlist(
    context: &DownloadContext<'_>,
    url: &str,
) -> Result<(String, Url), DownloadError> {
    let response = send_get(context, url).await?;
    let final_url = response.url().clone();
    let body = read_capped(response, MAX_PLAYLIST_BYTES, context.cancel).await?;
    let text = String::from_utf8_lossy(&body);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if !text.starts_with("#EXTM3U") {
        return Err(DownloadError::Message(
            "The server did not return an HLS playlist.".to_string(),
        ));
    }
    Ok((text.to_string(), final_url))
}

async fn read_capped(
    response: reqwest::Response,
    cap: usize,
    cancel: &CancelToken,
) -> Result<Vec<u8>, DownloadError> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    loop {
        let next = tokio::select! {
            next = stream.next() => next,
            _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
        };
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(|error| DownloadError::Message(reqwest_message(&error)))?;
        if body.len().saturating_add(chunk.len()) > cap {
            return Err(DownloadError::Message(
                "The HLS playlist is too large.".to_string(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn is_master_playlist(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim_start().starts_with("#EXT-X-STREAM-INF:"))
}

fn validate_master_tags(text: &str) -> Result<(), String> {
    for raw in text.lines() {
        let line = raw.trim();
        check_encryption(line)?;
        let Some(attributes) = line.strip_prefix("#EXT-X-MEDIA:") else {
            continue;
        };
        let media_type = attr_value(attributes, "TYPE").unwrap_or_default();
        let has_uri = attr_value(attributes, "URI").is_some_and(|uri| !uri.trim().is_empty());
        if media_type.eq_ignore_ascii_case("AUDIO") && has_uri {
            return Err(
                "This HLS stream uses a separate audio track. Saving it as one file is not supported."
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn parse_master_playlist(
    base: &Url,
    text: &str,
    proxy: &ProxyStore,
) -> Result<Vec<HlsVariant>, String> {
    let mut variants = Vec::new();
    let mut pending_bandwidth: Option<u64> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if let Some(attributes) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let bandwidth = attr_value(attributes, "BANDWIDTH")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            pending_bandwidth = Some(bandwidth);
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(bandwidth) = pending_bandwidth.take() else {
            continue;
        };
        let url = resolve_playlist_url(base, line, proxy)?;
        variants.push(HlsVariant {
            bandwidth,
            url: url.to_string(),
        });
    }

    if variants.is_empty() {
        return Err("The HLS master playlist has no variants.".to_string());
    }
    Ok(variants)
}

fn parse_media_playlist(base: &Url, text: &str, proxy: &ProxyStore) -> Result<HlsPlan, String> {
    let mut segments = Vec::new();
    let mut init: Option<String> = None;
    let mut byte_range_pending = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        check_encryption(line)?;
        if let Some(attributes) = line.strip_prefix("#EXT-X-MAP:") {
            if attr_value(attributes, "BYTERANGE").is_some() {
                return Err(
                    "This HLS playlist uses byte range initialization. Downloads are not supported."
                        .to_string(),
                );
            }
            let Some(uri) = attr_value(attributes, "URI") else {
                return Err(
                    "The HLS playlist has an initialization section without a URI.".to_string(),
                );
            };
            let resolved = resolve_playlist_url(base, &uri, proxy)?.to_string();
            if init.as_deref().is_some_and(|existing| existing != resolved) {
                return Err(
                    "This HLS playlist changes its initialization section. It cannot be saved as one file."
                        .to_string(),
                );
            }
            init = Some(resolved);
            continue;
        }
        if line.starts_with("#EXT-X-BYTERANGE") {
            byte_range_pending = true;
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if byte_range_pending {
            return Err(
                "This HLS playlist uses byte range segments. Downloads are not supported."
                    .to_string(),
            );
        }
        let resolved = resolve_playlist_url(base, line, proxy)?;
        if segments.len() >= MAX_HLS_SEGMENTS {
            return Err(format!(
                "The HLS playlist has more than {MAX_HLS_SEGMENTS} segments."
            ));
        }
        segments.push(resolved.to_string());
    }

    if segments.is_empty() {
        return Err("This HLS playlist has no media segments.".to_string());
    }
    let extension = hls_extension(init.as_deref(), &segments);
    Ok(HlsPlan {
        segments,
        init,
        extension,
    })
}

fn check_encryption(line: &str) -> Result<(), String> {
    let Some((tag, attributes)) = line.split_once(':') else {
        return Ok(());
    };
    if !tag.eq_ignore_ascii_case("#EXT-X-KEY") && !tag.eq_ignore_ascii_case("#EXT-X-SESSION-KEY") {
        return Ok(());
    }
    let method = attr_value(attributes, "METHOD").unwrap_or_default();
    if method.eq_ignore_ascii_case("NONE") {
        return Ok(());
    }
    let label = if method.trim().is_empty() {
        "unknown"
    } else {
        method.as_str()
    };
    Err(format!(
        "This HLS stream is encrypted ({label}). Encrypted downloads are not supported."
    ))
}

fn attr_value(attributes: &str, key: &str) -> Option<String> {
    for part in split_attributes(attributes) {
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(key) {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn split_attributes(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut quoted = false;
    for (index, character) in input.char_indices() {
        match character {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(&input[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

fn resolve_playlist_url(base: &Url, raw: &str, proxy: &ProxyStore) -> Result<Url, String> {
    let url = base
        .join(raw.trim())
        .map_err(|_| "The HLS playlist has an invalid URI.".to_string())?;
    if proxy.authorizes_url(url.as_str()) {
        return Ok(url);
    }
    safe_http::validate_https_url(&url)
        .map_err(|_| "The HLS playlist points to a non-public HTTPS URL.".to_string())?;
    Ok(url)
}

fn hls_extension(init: Option<&str>, segments: &[String]) -> &'static str {
    if init.is_some() {
        return "mp4";
    }
    let fmp4 = segments.iter().any(|segment| {
        let path = segment.split(['?', '#']).next().unwrap_or("");
        let lower = path.to_ascii_lowercase();
        lower.ends_with(".mp4") || lower.ends_with(".m4s") || lower.ends_with(".m4a")
    });
    if fmp4 {
        "mp4"
    } else {
        "ts"
    }
}

fn sanitize_file_name(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("The file name is empty.".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("The file name must not contain path separators.".to_string());
    }
    if trimmed.contains("..") {
        return Err("The file name must not contain two dots in a row.".to_string());
    }

    let mut cleaned = String::with_capacity(trimmed.len());
    for character in trimmed.chars() {
        if character.is_control() || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
            cleaned.push(' ');
        } else {
            cleaned.push(character);
        }
    }
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.replace(" .", ".");
    let cleaned = cleaned.trim_matches([' ', '.']).to_string();
    if cleaned.is_empty() {
        return Err("The file name has no usable characters.".to_string());
    }
    Ok(truncate_file_name(&cleaned))
}

fn is_media_extension(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "mp4"
            | "m4v"
            | "mov"
            | "webm"
            | "mkv"
            | "ogv"
            | "ogg"
            | "ts"
            | "m3u8"
            | "m4s"
            | "avi"
            | "flv"
            | "wmv"
    )
}

fn truncate_file_name(name: &str) -> String {
    if name.chars().count() <= MAX_FILE_NAME_CHARS {
        return name.to_string();
    }
    let path = Path::new(name);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| is_media_extension(value));
    let (stem, keep) = match extension {
        Some(extension) => (
            path.file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(name),
            MAX_FILE_NAME_CHARS.saturating_sub(extension.chars().count() + 1),
        ),
        None => (name, MAX_FILE_NAME_CHARS),
    };
    let short = stem.chars().take(keep).collect::<String>();
    let short = short.trim_end_matches([' ', '.']);
    match extension {
        Some(extension) => format!("{short}.{extension}"),
        None => short.to_string(),
    }
}

fn direct_file_name(file_name: &str, url: &str, kind: &str) -> Result<String, String> {
    let safe = sanitize_file_name(file_name)?;
    let has_extension = Path::new(&safe)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(is_media_extension);
    if has_extension {
        return Ok(safe);
    }
    let fallback = match kind {
        "ogv" | "ogg" => "ogv",
        "mp4" => "mp4",
        _ => url_extension(url).unwrap_or("mp4"),
    };
    Ok(format!("{safe}.{fallback}"))
}

fn hls_file_name(file_name: &str, target_extension: &str) -> Result<String, String> {
    let safe = sanitize_file_name(file_name)?;
    let path = Path::new(&safe);
    let has_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(is_media_extension);
    let stem = if has_extension {
        path.file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or(&safe)
    } else {
        &safe
    };
    Ok(format!("{stem}.{target_extension}"))
}

fn url_extension(url: &str) -> Option<&'static str> {
    let path = url.split(['?', '#']).next().unwrap_or("");
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "mp4" => Some("mp4"),
        "ogv" => Some("ogv"),
        "ogg" => Some("ogg"),
        "webm" => Some("webm"),
        _ => None,
    }
}

fn unique_destination(folder: &Path, file_name: &str) -> Result<(PathBuf, PathBuf), String> {
    let path = Path::new(file_name);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| is_media_extension(value));
    let stem = if extension.is_some() {
        path.file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(file_name)
    } else {
        file_name
    };

    for index in 0..MAX_COLLISION_TRIES {
        let candidate = match (index, extension) {
            (0, Some(extension)) => format!("{stem}.{extension}"),
            (0, None) => stem.to_string(),
            (_, Some(extension)) => format!("{stem} ({index}).{extension}"),
            (_, None) => format!("{stem} ({index})"),
        };
        let target = folder.join(&candidate);
        let part = folder.join(format!("{candidate}.part"));
        if !target.exists() && !part.exists() {
            return Ok((target, part));
        }
    }

    Err("No free file name was found in the downloads folder.".to_string())
}

fn new_download_id() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| format!("random source failed: {error}"))?;
    Ok(hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    fn no_proxy() -> ProxyStore {
        ProxyStore::for_tests(1)
    }

    fn test_folder(label: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "kaminari-download-{label}-{}",
            new_download_id().unwrap()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn stream_urls_must_be_public_https() {
        let proxy = no_proxy();
        assert!(validate_url("http://cdn.example.com/a.mp4", &proxy).is_err());
        assert!(validate_url("https://127.0.0.1/a.mp4", &proxy).is_err());
        assert!(validate_url("https://192.168.1.5/a.mp4", &proxy).is_err());
        assert!(validate_url("https://user:pass@cdn.example.com/a.mp4", &proxy).is_err());
        assert!(validate_url("https://cdn.example.com/a.mp4", &proxy).is_ok());
        assert!(validate_url("http://127.0.0.1:9/s/abc/file?token=def", &proxy).is_err());
    }

    #[test]
    fn playlist_children_must_be_public_https() {
        let base = url("https://cdn.example.com/hls/index.m3u8");
        let proxy = no_proxy();
        assert!(resolve_playlist_url(&base, "http://cdn.example.com/seg.ts", &proxy).is_err());
        assert!(resolve_playlist_url(&base, "https://127.0.0.1/seg.ts", &proxy).is_err());
        assert!(resolve_playlist_url(&base, "../seg.ts", &proxy).is_ok());
    }

    #[test]
    fn file_name_rejects_separators_and_parent_segments() {
        assert!(sanitize_file_name("../Show 01.mp4").is_err());
        assert!(sanitize_file_name("Show/01.mp4").is_err());
        assert!(sanitize_file_name("Show\\01.mp4").is_err());
        assert!(sanitize_file_name("Show..01.mp4").is_err());
    }

    #[test]
    fn file_name_replaces_invalid_characters() {
        assert_eq!(sanitize_file_name("Show: 01?*.mp4").unwrap(), "Show 01.mp4");
        assert_eq!(
            sanitize_file_name("  Show\n01.mp4  ").unwrap(),
            "Show 01.mp4"
        );
    }

    #[test]
    fn collision_adds_numeric_suffix() {
        let folder = test_folder("collision");
        std::fs::write(folder.join("Show 01.mp4"), b"old").unwrap();

        let (target, part) = unique_destination(&folder, "Show 01.mp4").unwrap();
        assert_eq!(
            target.file_name().unwrap().to_string_lossy(),
            "Show 01 (1).mp4"
        );
        assert_eq!(
            part.file_name().unwrap().to_string_lossy(),
            "Show 01 (1).mp4.part"
        );

        std::fs::write(folder.join("Show 01 (1).mp4"), b"old").unwrap();
        let (target, _) = unique_destination(&folder, "Show 01.mp4").unwrap();
        assert_eq!(
            target.file_name().unwrap().to_string_lossy(),
            "Show 01 (2).mp4"
        );

        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn hls_file_name_keeps_dots_in_the_title() {
        assert_eq!(
            hls_file_name("Dr. Stone - Episode 1 - 1080p - HLS", "ts").unwrap(),
            "Dr. Stone - Episode 1 - 1080p - HLS.ts"
        );
    }

    #[test]
    fn collision_keeps_dots_in_the_title() {
        let folder = test_folder("dotted");
        std::fs::write(folder.join("Dr. Stone - Episode 1 - HLS.ts"), b"old").unwrap();
        let (target, _) = unique_destination(&folder, "Dr. Stone - Episode 1 - HLS.ts").unwrap();
        assert_eq!(
            target.file_name().unwrap().to_string_lossy(),
            "Dr. Stone - Episode 1 - HLS (1).ts"
        );
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn direct_file_name_uses_stream_extension() {
        assert_eq!(
            direct_file_name("Show 01", "https://cdn.example.com/a.ogv", "ogv").unwrap(),
            "Show 01.ogv"
        );
        assert_eq!(
            direct_file_name("Show 01.mp4", "https://cdn.example.com/a.mp4", "mp4").unwrap(),
            "Show 01.mp4"
        );
    }

    #[test]
    fn hls_file_name_uses_playlist_extension() {
        assert_eq!(
            hls_file_name("Show 01 HLS", "ts").unwrap(),
            "Show 01 HLS.ts"
        );
        assert_eq!(hls_file_name("Show 01.mp4", "mp4").unwrap(), "Show 01.mp4");
    }

    #[test]
    fn master_playlist_picks_highest_bandwidth() {
        let base = url("https://cdn.example.com/show/master.m3u8");
        let text = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360\nlow/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2400000,RESOLUTION=1920x1080\nhigh/index.m3u8\n";
        assert!(validate_master_tags(text).is_ok());
        let variants = parse_master_playlist(&base, text, &no_proxy()).unwrap();
        let best = variants
            .into_iter()
            .max_by_key(|variant| variant.bandwidth)
            .unwrap();
        assert_eq!(best.bandwidth, 2_400_000);
        assert_eq!(best.url, "https://cdn.example.com/show/high/index.m3u8");
    }

    #[test]
    fn media_playlist_parses_and_resolves_segments() {
        let base = url("https://cdn.example.com/hls/show/index.m3u8");
        let text = "#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:4.0,\nseg0.ts\n#EXTINF:4.0,\n../seg1.ts\n#EXTINF:4.0,\nhttps://media.example.net/seg2.ts\n#EXT-X-ENDLIST\n";
        let plan = parse_media_playlist(&base, text, &no_proxy()).unwrap();
        assert_eq!(
            plan.segments,
            vec![
                "https://cdn.example.com/hls/show/seg0.ts".to_string(),
                "https://cdn.example.com/hls/seg1.ts".to_string(),
                "https://media.example.net/seg2.ts".to_string(),
            ]
        );
        assert_eq!(plan.extension, "ts");
        assert!(plan.init.is_none());
    }

    #[test]
    fn init_section_marks_mp4() {
        let base = url("https://cdn.example.com/hls/index.m3u8");
        let text = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:4.0,\nseg1.m4s\n#EXT-X-ENDLIST\n";
        let plan = parse_media_playlist(&base, text, &no_proxy()).unwrap();
        assert_eq!(
            plan.init.as_deref(),
            Some("https://cdn.example.com/hls/init.mp4")
        );
        assert_eq!(plan.extension, "mp4");
    }

    #[test]
    fn encrypted_playlist_is_rejected() {
        let base = url("https://cdn.example.com/hls/index.m3u8");
        let text = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n#EXTINF:4.0,\nseg.ts\n#EXT-X-ENDLIST\n";
        let error = parse_media_playlist(&base, text, &no_proxy()).unwrap_err();
        assert!(error.contains("encrypted"));
    }

    #[test]
    fn unencrypted_key_tag_is_allowed() {
        let base = url("https://cdn.example.com/hls/index.m3u8");
        let text = "#EXTM3U\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:4.0,\nseg.ts\n#EXT-X-ENDLIST\n";
        assert!(parse_media_playlist(&base, text, &no_proxy()).is_ok());
    }

    #[test]
    fn separate_audio_group_is_rejected() {
        let text = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"English\",URI=\"audio/en.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO=\"aud\"\nv.m3u8\n";
        let error = validate_master_tags(text).unwrap_err();
        assert!(error.contains("audio"));
    }
}
