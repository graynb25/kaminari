// Dev spike bridge for embedded libmpv. The Svelte flag decides whether this
// path is used; release playback stays on PlayerView. All commands are typed
// intents with a fixed allowlist. Raw mpv commands never come from the WebView.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_libmpv::{MpvConfig, MpvExt};

const ALLOWED_HEADERS: [&str; 7] = [
    "accept",
    "accept-language",
    "authorization",
    "cookie",
    "origin",
    "referer",
    "user-agent",
];
const MAX_HEADER_COUNT: usize = 16;
const MAX_HEADER_BYTES: usize = 8192;

#[derive(Default)]
pub struct MpvEmbedState {
    last_error: Mutex<Option<String>>,
    active_label: Mutex<Option<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpvEmbedStatus {
    pub available: bool,
    pub wrapper_present: bool,
    pub libmpv_present: bool,
    pub dll_dir: Option<String>,
    pub dev_dir: Option<String>,
    pub active_label: Option<String>,
    pub last_error: Option<String>,
}

fn dll_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
            dirs.push(parent.join("lib"));
        }
    }
    dirs
}

fn dll_state() -> (bool, bool, Option<PathBuf>) {
    for dir in dll_dirs() {
        let wrapper = dir.join("libmpv-wrapper.dll").is_file();
        let libmpv = dir.join("libmpv-2.dll").is_file();
        if wrapper && libmpv {
            return (true, true, Some(dir));
        }
    }
    let wrapper = dll_dirs()
        .iter()
        .any(|dir| dir.join("libmpv-wrapper.dll").is_file());
    let libmpv = dll_dirs()
        .iter()
        .any(|dir| dir.join("libmpv-2.dll").is_file());
    (wrapper, libmpv, None)
}

fn dev_lib_dir() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("lib");
    dir.is_dir().then(|| dir.display().to_string())
}

fn lock_string(lock: &Mutex<Option<String>>) -> Option<String> {
    lock.lock().ok().and_then(|guard| guard.clone())
}

fn set_last_error(state: &MpvEmbedState, message: Option<String>) {
    if let Ok(mut guard) = state.last_error.lock() {
        *guard = message;
    }
}

fn set_active(state: &MpvEmbedState, label: Option<String>) {
    if let Ok(mut guard) = state.active_label.lock() {
        *guard = label;
    }
}

fn redact(input: &str) -> String {
    input
        .split_whitespace()
        .map(|token| match token.find('?') {
            Some(index) if token.starts_with("http://") || token.starts_with("https://") => {
                format!("{}?<redacted>", &token[..index])
            }
            _ => token.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_authority(url: &str) -> Option<(&str, Option<&str>)> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    if let Some(stripped) = authority.strip_prefix('[') {
        let end = stripped.find(']')?;
        let host = &authority[..end + 2];
        let remainder = &stripped[end + 1..];
        if remainder.is_empty() {
            return Some((host, None));
        }
        return Some((host, Some(remainder.strip_prefix(':')?)));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) => Some((host, Some(port))),
        None => Some((authority, None)),
    }
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

fn loopback_proxy_port(url: &str) -> Option<u16> {
    let (host, port) = split_authority(url)?;
    if !is_loopback_host(host) {
        return None;
    }
    port?.parse::<u16>().ok()
}

fn validate_media_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err("The stream URL is empty.".to_string());
    }
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        let Some((host, port)) = split_authority(url) else {
            return Err("Only http and https media URLs are allowed.".to_string());
        };
        if is_loopback_host(host) {
            let port = port.and_then(|port| port.parse::<u16>().ok());
            if !matches!(port, Some(39321..=39328)) {
                return Err("Loopback media is limited to the app proxy.".to_string());
            }
        }
        return Ok(url.to_string());
    }

    // Dev spike only: a local file keeps the HEVC sample out of the repo.
    if cfg!(debug_assertions) {
        let path = Path::new(url);
        if path.is_absolute() && path.is_file() {
            return Ok(url.to_string());
        }
    }

    Err("Only http and https media URLs are allowed.".to_string())
}

fn validate_header_route(
    url: &str,
    headers: Option<&HashMap<String, String>>,
) -> Result<(), String> {
    let Some(headers) = headers else {
        return Ok(());
    };
    let has_headers = headers
        .iter()
        .any(|(key, value)| !key.trim().is_empty() && !value.trim().is_empty());
    if !has_headers {
        return Ok(());
    }
    if headers.iter().any(|(key, value)| {
        !key.trim().is_empty() && !value.trim().is_empty() && value.contains(',')
    }) {
        return Err(
            "Header values cannot contain commas. Use the loopback proxy for this stream."
                .to_string(),
        );
    }
    if !url.to_ascii_lowercase().starts_with("http://")
        || !matches!(loopback_proxy_port(url), Some(39321..=39328))
    {
        return Err(
            "Header protected streams must use the loopback proxy. Direct headers are not supported."
                .to_string(),
        );
    }
    Ok(())
}

fn sanitize_headers(headers: Option<&HashMap<String, String>>) -> Vec<String> {
    let Some(headers) = headers else {
        return Vec::new();
    };
    let mut entries = headers
        .iter()
        .filter_map(|(key, value)| {
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                return None;
            }
            if !ALLOWED_HEADERS
                .iter()
                .any(|allowed| key.eq_ignore_ascii_case(allowed))
            {
                return None;
            }
            let value = value.replace(['\r', '\n', '\0'], " ");
            Some((key.to_string(), value))
        })
        .collect::<Vec<_>>();
    entries.sort();

    let mut fields = Vec::new();
    let mut bytes = 0usize;
    for (key, value) in entries {
        let line = format!("{key}: {value}");
        bytes += line.len() + 2;
        if fields.len() >= MAX_HEADER_COUNT || bytes > MAX_HEADER_BYTES {
            break;
        }
        fields.push(line);
    }
    fields
}

fn check_property(name: &str, value: Value) -> Result<Value, String> {
    match name {
        "pause" | "mute" | "sub-visibility" => {
            if value.is_boolean() {
                Ok(value)
            } else {
                Err(format!("{name} needs a boolean value."))
            }
        }
        "speed" => {
            let speed = value
                .as_f64()
                .ok_or_else(|| "speed needs a number.".to_string())?;
            if (0.25..=4.0).contains(&speed) {
                Ok(value)
            } else {
                Err("speed must stay between 0.25 and 4.".to_string())
            }
        }
        "volume" => {
            let volume = value
                .as_f64()
                .ok_or_else(|| "volume needs a number.".to_string())?;
            if (0.0..=100.0).contains(&volume) {
                Ok(value)
            } else {
                Err("volume must stay between 0 and 100.".to_string())
            }
        }
        "sid" | "aid" => {
            if value.is_string() || value.is_number() {
                Ok(value)
            } else {
                Err(format!("{name} needs a track id or a name."))
            }
        }
        _ => Err(format!("Property {name} is not allowed.")),
    }
}

fn property_format(name: &str) -> Result<&'static str, String> {
    match name {
        "hwdec-current" | "video-codec" | "video-format" | "current-vo" => Ok("string"),
        "width" | "height" | "wid" => Ok("int64"),
        "vo-configured" | "eof-reached" => Ok("flag"),
        "time-pos" | "duration" => Ok("double"),
        _ => Err(format!("Property {name} is not readable.")),
    }
}

#[tauri::command]
pub fn mpv_embed_status(state: State<'_, MpvEmbedState>) -> MpvEmbedStatus {
    let (wrapper_present, libmpv_present, dll_dir) = dll_state();
    MpvEmbedStatus {
        available: wrapper_present && libmpv_present,
        wrapper_present,
        libmpv_present,
        dll_dir: dll_dir.map(|dir| dir.display().to_string()),
        dev_dir: dev_lib_dir(),
        active_label: lock_string(&state.active_label),
        last_error: lock_string(&state.last_error),
    }
}

#[tauri::command]
pub async fn mpv_embed_init(
    app: AppHandle,
    state: State<'_, MpvEmbedState>,
    window_label: String,
) -> Result<String, String> {
    if app.get_webview_window(&window_label).is_none() {
        return Err("The target window is not open.".to_string());
    }
    let (wrapper_present, libmpv_present, _) = dll_state();
    if !wrapper_present || !libmpv_present {
        let message = "libmpv DLLs are missing. Falling back to the WebView player.";
        set_last_error(&state, Some(message.to_string()));
        return Err(message.to_string());
    }

    let config: MpvConfig = serde_json::from_value(json!({
        "initialOptions": {
            "vo": "gpu-next",
            "hwdec": "auto-safe",
            "keep-open": "yes",
            "idle": "yes",
            "force-window": "yes",
            "input-default-bindings": "no",
            "input-vo-keyboard": "no",
            "osc": "no",
            "osd-level": "0",
            "sub-auto": "fuzzy",
            "subs-fallback": "yes",
            "terminal": "no",
            "msg-level": "all=warn"
        },
        "observedProperties": {
            "pause": "flag",
            "time-pos": "double",
            "duration": "double",
            "eof-reached": "flag",
            "hwdec-current": "string",
            "video-format": "string",
            "video-codec": "string"
        }
    }))
    .map_err(|err| format!("mpv config is invalid: {err}"))?;

    // A hard reload can skip the Svelte unmount cleanup. Tear down any old
    // handle for this label before creating a fresh one.
    if let Some(previous) = lock_string(&state.active_label) {
        let _ = app.mpv().destroy(&previous);
    }

    let init_app = app.clone();
    let init_label = window_label.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || init_app.mpv().init(config, &init_label))
            .await
            .map_err(|err| format!("mpv init task failed: {err}"))?;

    match result {
        Ok(label) => {
            set_active(&state, Some(label.clone()));
            set_last_error(&state, None);
            tracing::info!(window = %label, "mpv embed: initialized");
            Ok(label)
        }
        Err(err) => {
            let message = format!("mpv could not start: {}", redact(&err.to_string()));
            set_last_error(&state, Some(message.clone()));
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn mpv_embed_load(
    app: AppHandle,
    state: State<'_, MpvEmbedState>,
    window_label: String,
    url: String,
    headers: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let media_url = validate_media_url(&url)?;
    validate_header_route(&media_url, headers.as_ref())?;
    let fields = sanitize_headers(headers.as_ref());
    let load_app = app.clone();
    let load_label = window_label.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let mpv = load_app.mpv();
        // The wrapper cannot send a JSON array yet. mpv parses this option as
        // a comma separated string, so join the sanitized allowlist. Leave it
        // alone when empty instead of sending an unsupported value type.
        if !fields.is_empty() {
            mpv.set_property("http-header-fields", &json!(fields.join(",")), &load_label)
                .map_err(|err| redact(&err.to_string()))?;
        }
        mpv.command(
            "loadfile",
            &vec![json!(media_url), json!("replace")],
            &load_label,
        )
        .map_err(|err| redact(&err.to_string()))
    })
    .await
    .map_err(|err| format!("mpv load task failed: {err}"))?;

    match result {
        Ok(()) => {
            set_last_error(&state, None);
            Ok(())
        }
        Err(err) => {
            let message = format!("mpv could not load the stream: {err}");
            set_last_error(&state, Some(message.clone()));
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn mpv_embed_set(
    app: AppHandle,
    state: State<'_, MpvEmbedState>,
    window_label: String,
    name: String,
    value: Value,
) -> Result<(), String> {
    let value = check_property(&name, value)?;
    let set_app = app.clone();
    let set_label = window_label.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        set_app.mpv().set_property(&name, &value, &set_label)
    })
    .await
    .map_err(|err| format!("mpv property task failed: {err}"))?;
    result.map_err(|err| {
        let message = format!("mpv property failed: {}", redact(&err.to_string()));
        set_last_error(&state, Some(message.clone()));
        message
    })
}

#[tauri::command]
pub async fn mpv_embed_seek(
    app: AppHandle,
    state: State<'_, MpvEmbedState>,
    window_label: String,
    seconds: f64,
    mode: String,
) -> Result<(), String> {
    if !seconds.is_finite() {
        return Err("The seek target is not finite.".to_string());
    }
    if mode != "absolute" && mode != "relative" {
        return Err("Seek mode must be absolute or relative.".to_string());
    }
    let target = if mode == "absolute" {
        seconds.max(0.0)
    } else {
        seconds
    };
    let seek_app = app.clone();
    let seek_label = window_label.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        seek_app
            .mpv()
            .command("seek", &vec![json!(target), json!(mode)], &seek_label)
    })
    .await
    .map_err(|err| format!("mpv seek task failed: {err}"))?;
    result.map_err(|err| {
        let message = format!("mpv seek failed: {}", redact(&err.to_string()));
        set_last_error(&state, Some(message.clone()));
        message
    })
}

#[tauri::command]
pub async fn mpv_embed_cycle_sub(
    app: AppHandle,
    state: State<'_, MpvEmbedState>,
    window_label: String,
) -> Result<(), String> {
    let cycle_app = app.clone();
    let cycle_label = window_label.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        cycle_app
            .mpv()
            .command("cycle", &vec![json!("sub")], &cycle_label)
    })
    .await
    .map_err(|err| format!("mpv subtitle task failed: {err}"))?;
    result.map_err(|err| {
        let message = format!("mpv subtitle failed: {}", redact(&err.to_string()));
        set_last_error(&state, Some(message.clone()));
        message
    })
}

#[tauri::command]
pub async fn mpv_embed_get(
    app: AppHandle,
    window_label: String,
    name: String,
) -> Result<Value, String> {
    let format = property_format(&name)?;
    let get_app = app.clone();
    let get_label = window_label.clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_app
            .mpv()
            .get_property(name, format.to_string(), &get_label)
    })
    .await
    .map_err(|err| format!("mpv property task failed: {err}"))?
    .map_err(|err| format!("mpv property failed: {}", redact(&err.to_string())))
}

#[tauri::command]
pub async fn mpv_embed_destroy(
    app: AppHandle,
    state: State<'_, MpvEmbedState>,
    window_label: String,
) -> Result<(), String> {
    let destroy_app = app.clone();
    let destroy_label = window_label.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || destroy_app.mpv().destroy(&destroy_label))
            .await
            .map_err(|err| format!("mpv destroy task failed: {err}"))?;
    if lock_string(&state.active_label).as_deref() == Some(window_label.as_str()) {
        set_active(&state, None);
    }
    result.map_err(|err| {
        let message = format!("mpv could not stop: {}", redact(&err.to_string()));
        set_last_error(&state, Some(message.clone()));
        message
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_schemes() {
        assert!(validate_media_url("file:///C:/clip.mp4").is_err());
        assert!(validate_media_url("data:video/mp4;base64,AAAA").is_err());
        assert!(validate_media_url("blob:https://example.test/1").is_err());
    }

    #[test]
    fn accepts_http_and_app_proxy() {
        assert_eq!(
            validate_media_url("https://example.test/ep1.mp4").unwrap(),
            "https://example.test/ep1.mp4"
        );
        assert!(validate_media_url("http://127.0.0.1:39321/s/abc/file").is_ok());
        assert!(validate_media_url("http://127.0.0.1:8080/s/abc/file").is_err());
        assert!(validate_media_url("http://localhost:39324/s/abc/file").is_ok());
    }

    #[test]
    fn rejects_headers_on_direct_streams() {
        let headers = HashMap::from([("Referer".to_string(), "https://source.test/".to_string())]);
        let error = validate_header_route("https://cdn.test/ep1.mp4", Some(&headers)).unwrap_err();
        assert!(error.contains("loopback proxy"));
        assert!(!error.contains("https://cdn.test"));
    }

    #[test]
    fn allows_headers_on_the_app_proxy() {
        let headers = HashMap::from([("Referer".to_string(), "https://source.test/".to_string())]);
        assert!(validate_header_route(
            "http://127.0.0.1:39321/s/abc/file?token=def",
            Some(&headers)
        )
        .is_ok());
        assert!(validate_header_route(
            "http://localhost:39328/s/abc/manifest.m3u8?token=def",
            Some(&headers)
        )
        .is_ok());
        assert!(validate_header_route(
            "http://127.0.0.1:39329/s/abc/file?token=def",
            Some(&headers)
        )
        .is_err());
        assert!(validate_header_route(
            "https://127.0.0.1:39321/s/abc/file?token=def",
            Some(&headers)
        )
        .is_err());
    }

    #[test]
    fn rejects_header_values_with_commas() {
        let headers = HashMap::from([("Cookie".to_string(), "session=one,theme=dark".to_string())]);
        let error = validate_header_route(
            "http://127.0.0.1:39321/s/abc/file?token=def",
            Some(&headers),
        )
        .unwrap_err();
        assert!(error.contains("commas"));
        assert!(error.contains("loopback proxy"));
    }

    #[test]
    fn allows_calls_without_headers() {
        assert!(validate_header_route("https://cdn.test/ep1.mp4", None).is_ok());
        assert!(validate_header_route("https://cdn.test/ep1.mp4", Some(&HashMap::new())).is_ok());
        let blank = HashMap::from([("Referer".to_string(), "   ".to_string())]);
        assert!(validate_header_route("https://cdn.test/ep1.mp4", Some(&blank)).is_ok());
    }

    #[test]
    fn sanitizes_headers_to_the_allowlist() {
        let headers = HashMap::from([
            (
                "Referer".to_string(),
                "https://source.test/\r\nInjected: 1".to_string(),
            ),
            ("X-Not-Allowed".to_string(), "no".to_string()),
            ("User-Agent".to_string(), "KaminariTest".to_string()),
            ("Cookie".to_string(), "session=one\ntwo".to_string()),
        ]);
        let fields = sanitize_headers(Some(&headers));
        assert_eq!(fields.len(), 3);
        assert!(fields.contains(&"Referer: https://source.test/  Injected: 1".to_string()));
        assert!(fields.contains(&"User-Agent: KaminariTest".to_string()));
        assert!(fields.contains(&"Cookie: session=one two".to_string()));
        assert!(!fields.iter().any(|line| line.contains("X-Not-Allowed")));
    }

    #[test]
    fn caps_header_count_and_bytes() {
        let mut headers = HashMap::new();
        for index in 0..40 {
            headers.insert(format!("Accept-{index}"), "x".repeat(64));
        }
        // Only names in the allowlist can add a header, so the cap is tested
        // through a realistic mix.
        headers.insert("Accept".to_string(), "x".repeat(MAX_HEADER_BYTES + 1));
        let fields = sanitize_headers(Some(&headers));
        assert!(fields.len() <= 1);
    }

    #[test]
    fn redacts_query_strings() {
        let text = "failed https://cdn.test/ep1.mp4?token=secret after 2s";
        assert_eq!(
            redact(text),
            "failed https://cdn.test/ep1.mp4?<redacted> after 2s"
        );
    }

    #[test]
    fn property_allowlist_is_typed() {
        assert!(check_property("pause", json!(true)).is_ok());
        assert!(check_property("pause", json!(1)).is_err());
        assert!(check_property("speed", json!(2.0)).is_ok());
        assert!(check_property("speed", json!(9.0)).is_err());
        assert!(check_property("volume", json!(80)).is_ok());
        assert!(check_property("volume", json!(120)).is_err());
        assert!(check_property("sid", json!("auto")).is_ok());
        assert!(check_property("raw", json!("no")).is_err());
        assert!(property_format("hwdec-current").is_ok());
        assert!(property_format("options").is_err());
    }
}
