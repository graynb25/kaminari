// Native JavaScript extension host. One QuickJS runtime per call.
// The bridge is the only way out. See docs/PLAN-JS-EXTENSIONS.md.

use std::collections::HashMap;
use std::io::Read;
use std::net::IpAddr;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rquickjs::{Context, Ctx, Function, Object, Promise, Runtime};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub const TIMEOUT_SECS: u64 = 5;
pub const RESOLVE_TIMEOUT_SECS: u64 = 10;
pub const COMPAT_TIMEOUT_SECS: u64 = 60;
pub const COMPAT_RESOLVE_TIMEOUT_SECS: u64 = 120;
pub const MEMORY_BYTES: usize = 64 * 1024 * 1024;
pub const STACK_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const MAX_CODE_BYTES: usize = 2 * 1024 * 1024;
const COMPAT_FETCH_TIMEOUT_SECS: u64 = 30;
const MAX_FETCH_TIMEOUT_SECS: u64 = 60;
// html.select runs in Rust, outside the QuickJS memory limit. Cap every field
// and the whole result; strings are cut on UTF-8 boundaries.
const MAX_SELECT_NODES: usize = 500;
const MAX_SELECT_TEXT_BYTES: usize = 64 * 1024;
const MAX_SELECT_HTML_BYTES: usize = 256 * 1024;
const MAX_SELECT_ATTRS: usize = 64;
const MAX_SELECT_ATTR_NAME_BYTES: usize = 256;
const MAX_SELECT_OUTPUT_BYTES: usize = 1024 * 1024;
const SELECT_ROW_OVERHEAD_BYTES: usize = 512;
#[cfg(test)]
const MAX_STORAGE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_KEXT_BYTES: usize = 4 * 1024 * 1024;

pub fn validate_source_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())
}

pub fn package_from_kext(bytes: &[u8]) -> AppResult<ExtPackage> {
    if bytes.len() > MAX_KEXT_BYTES {
        return Err(AppError::Extension(
            "package is larger than 4 MB".to_string(),
        ));
    }
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|err| AppError::Extension(format!("invalid zip: {err}")))?;
    if archive.len() > 64 {
        return Err(AppError::Extension(
            "package has more than 64 entries".to_string(),
        ));
    }

    let mut manifest_raw: Option<String> = None;
    let mut code: Option<String> = None;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| AppError::Extension(format!("invalid zip entry: {err}")))?;
        let name = entry
            .name()
            .map_err(|err| AppError::Extension(format!("invalid zip entry name: {err}")))?
            .into_owned();
        match name.as_str() {
            "manifest.json" => {
                if entry.size() > 64 * 1024 {
                    return Err(AppError::Extension(
                        "manifest.json is larger than 64 KB".to_string(),
                    ));
                }
                let mut raw = String::new();
                entry
                    .read_to_string(&mut raw)
                    .map_err(|err| AppError::Extension(err.to_string()))?;
                manifest_raw = Some(raw);
            }
            "index.js" => {
                if entry.size() > MAX_CODE_BYTES as u64 {
                    return Err(AppError::Extension(
                        "index.js is larger than 2 MB".to_string(),
                    ));
                }
                let mut raw = String::new();
                entry
                    .read_to_string(&mut raw)
                    .map_err(|err| AppError::Extension(err.to_string()))?;
                code = Some(raw);
            }
            _ => {}
        }
    }

    let manifest_raw =
        manifest_raw.ok_or_else(|| AppError::Extension("manifest.json is missing".to_string()))?;
    let code = code.ok_or_else(|| AppError::Extension("index.js is missing".to_string()))?;
    let manifest: Manifest = serde_json::from_str(&manifest_raw)
        .map_err(|err| AppError::Extension(format!("manifest.json is invalid: {err}")))?;
    manifest.validate()?;
    Ok(ExtPackage { manifest, code })
}

pub fn sample_package() -> AppResult<ExtPackage> {
    let manifest: Manifest =
        serde_json::from_str(include_str!("../../extensions/public-domain/manifest.json"))
            .map_err(|err| AppError::Extension(format!("sample manifest is invalid: {err}")))?;
    manifest.validate()?;
    Ok(ExtPackage {
        manifest,
        code: include_str!("../../extensions/public-domain/index.js").to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    pub hosts: Vec<String>,
    #[serde(default)]
    pub storage_kb: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub entry: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub min_host: String,
    #[serde(default)]
    pub nsfw: bool,
    #[serde(default)]
    pub icon: Option<String>,
    pub permissions: Permissions,
}

impl Manifest {
    pub fn validate(&self) -> AppResult<()> {
        if self.id.len() < 3 || self.id.len() > 128 {
            return Err(invalid_manifest("id must be 3 to 128 characters"));
        }
        if !self
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        {
            return Err(invalid_manifest("id contains an unsupported character"));
        }
        if self.name.trim().is_empty() {
            return Err(invalid_manifest("name is required"));
        }
        if self.version.trim().is_empty() {
            return Err(invalid_manifest("version is required"));
        }
        if self.api_version != "0.1" {
            return Err(invalid_manifest("apiVersion must be 0.1"));
        }
        if self.entry != "index.js" {
            return Err(invalid_manifest("entry must be index.js in v0.1"));
        }
        if self.permissions.hosts.is_empty() {
            return Err(invalid_manifest(
                "permissions.hosts needs at least one host",
            ));
        }
        if self.permissions.storage_kb.unwrap_or(512) > 1024 {
            return Err(invalid_manifest("storageKb cannot exceed 1024"));
        }
        if self.allowed_rules()?.is_empty() {
            return Err(invalid_manifest("no usable hosts in permissions.hosts"));
        }
        Ok(())
    }

    pub(crate) fn allowed_rules(&self) -> AppResult<Vec<HostRule>> {
        let mut rules = Vec::new();
        for raw in &self.permissions.hosts {
            if let Some(rule) = HostRule::parse(raw) {
                rules.push(rule);
            }
        }
        Ok(rules)
    }
}

#[derive(Debug, Clone)]
pub struct ExtPackage {
    pub manifest: Manifest,
    pub code: String,
}

#[derive(Debug, Clone, Copy)]
pub struct HostLimits {
    pub timeout: Duration,
    pub resolve_timeout: Duration,
    pub memory_bytes: usize,
    pub stack_bytes: usize,
}

impl Default for HostLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(TIMEOUT_SECS),
            resolve_timeout: Duration::from_secs(RESOLVE_TIMEOUT_SECS),
            memory_bytes: MEMORY_BYTES,
            stack_bytes: STACK_BYTES,
        }
    }
}

pub trait ExtStorage: Send + Sync {
    fn get(&self, extension_id: &str, key: &str) -> Option<String>;
    fn set(&self, extension_id: &str, key: &str, value: &str) -> AppResult<()>;
    fn remove(&self, extension_id: &str, key: &str) -> AppResult<()>;
}

#[cfg(test)]
#[derive(Default)]
pub struct MemoryStorage {
    map: Mutex<HashMap<(String, String), String>>,
}

#[cfg(test)]
impl ExtStorage for MemoryStorage {
    fn get(&self, extension_id: &str, key: &str) -> Option<String> {
        let map = self.map.lock().ok()?;
        map.get(&(extension_id.to_string(), key.to_string()))
            .cloned()
    }

    fn set(&self, extension_id: &str, key: &str, value: &str) -> AppResult<()> {
        let mut map = self
            .map
            .lock()
            .map_err(|_| AppError::Extension("storage lock poisoned".to_string()))?;
        let used: usize = map
            .iter()
            .filter(|((id, _), _)| id == extension_id)
            .map(|((key, value), _)| key.len() + value.len())
            .sum();
        if used + key.len() + value.len() > MAX_STORAGE_BYTES {
            return Err(AppError::Extension(
                "storage limit reached (1 MB per extension)".to_string(),
            ));
        }
        map.insert(
            (extension_id.to_string(), key.to_string()),
            value.to_string(),
        );
        Ok(())
    }

    fn remove(&self, extension_id: &str, key: &str) -> AppResult<()> {
        let mut map = self
            .map
            .lock()
            .map_err(|_| AppError::Extension("storage lock poisoned".to_string()))?;
        map.remove(&(extension_id.to_string(), key.to_string()));
        Ok(())
    }
}

struct HostContext {
    extension_id: String,
    allow: Vec<HostRule>,
    storage: Arc<dyn ExtStorage>,
    fetch_timeout: Duration,
    deadline: Instant,
}

#[derive(Debug, Clone)]
pub(crate) struct HostRule {
    host: String,
    path_prefix: String,
}

impl HostRule {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw == "*" {
            return Some(Self {
                host: "*".to_string(),
                path_prefix: "/".to_string(),
            });
        }
        let trimmed = raw.trim_end_matches('*');
        let url = reqwest::Url::parse(trimmed).ok()?;
        if url.scheme() != "https" {
            return None;
        }
        let host = url.host_str()?.to_ascii_lowercase();
        let path = url.path();
        Some(Self {
            host,
            path_prefix: if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            },
        })
    }

    pub(crate) fn allows(&self, url: &reqwest::Url) -> bool {
        if url.scheme() != "https" {
            return false;
        }
        if self.host == "*" {
            return true;
        }
        url.host_str()
            .map(|host| host.eq_ignore_ascii_case(&self.host))
            .unwrap_or(false)
            && url.path().starts_with(&self.path_prefix)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchOptions {
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug)]
struct HostError {
    code: &'static str,
    message: String,
}

impl HostError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn json(&self) -> String {
        json!({ "ok": false, "code": self.code, "message": self.message }).to_string()
    }
}

pub fn run(
    package: &ExtPackage,
    op: &str,
    args: Value,
    storage: Arc<dyn ExtStorage>,
    limits: &HostLimits,
) -> AppResult<Value> {
    package.manifest.validate()?;
    if package.code.len() > MAX_CODE_BYTES {
        return Err(AppError::Extension(
            "extension code is larger than 2 MB".to_string(),
        ));
    }
    let compat = package.manifest.id.starts_with("mangayomi.");
    let timeout = match op {
        "search" | "getShow" | "getEpisodes" | "getPreferences" => {
            if compat {
                Duration::from_secs(COMPAT_TIMEOUT_SECS)
            } else {
                limits.timeout
            }
        }
        "resolveEpisode" => {
            if compat {
                Duration::from_secs(COMPAT_RESOLVE_TIMEOUT_SECS)
            } else {
                limits.resolve_timeout
            }
        }
        other => {
            return Err(AppError::Extension(format!(
                "unknown extension operation: {other}"
            )))
        }
    };

    let deadline = Instant::now() + timeout;
    let host = Arc::new(HostContext {
        extension_id: package.manifest.id.clone(),
        allow: package.manifest.allowed_rules()?,
        storage,
        fetch_timeout: Duration::from_secs(if compat {
            COMPAT_FETCH_TIMEOUT_SECS
        } else {
            10
        }),
        deadline,
    });

    let runtime = Runtime::new().map_err(|err| AppError::Extension(err.to_string()))?;
    runtime.set_memory_limit(limits.memory_bytes);
    runtime.set_max_stack_size(limits.stack_bytes);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));

    let context = Context::full(&runtime).map_err(|err| AppError::Extension(err.to_string()))?;
    let result = context.with(|ctx| {
        inject_bridge(&ctx, host)?;
        ctx.eval::<(), _>(BRIDGE_SHIM)
            .map_err(|err| AppError::Extension(err.to_string()))?;
        ctx.eval::<(), _>(package.code.as_str())
            .map_err(|err| AppError::Extension(err.to_string()))?;

        let globals = ctx.globals();
        globals
            .set("__kaminari_op", op.to_string())
            .map_err(|err| AppError::Extension(err.to_string()))?;
        globals
            .set("__kaminari_args", args.to_string())
            .map_err(|err| AppError::Extension(err.to_string()))?;

        let promise: Promise = ctx
            .eval(DISPATCH)
            .map_err(|err| AppError::Extension(err.to_string()))?;
        let raw = finish_promise(&ctx, promise, deadline, timeout)?;
        let envelope: Value = serde_json::from_str(&raw).map_err(|err| {
            AppError::Extension(format!("extension returned invalid JSON: {err}"))
        })?;
        if envelope.get("ok").and_then(Value::as_bool) == Some(false) {
            let code = envelope
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("EXTENSION_ERROR");
            let message = envelope
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("extension failed");
            return Err(AppError::Extension(format!("{code}: {message}")));
        }
        Ok(envelope.get("value").cloned().unwrap_or(Value::Null))
    });

    if result.is_err() && Instant::now() >= deadline {
        return Err(AppError::ExtensionTimeout(timeout.as_secs()));
    }
    result
}

fn finish_promise(
    ctx: &Ctx<'_>,
    promise: Promise<'_>,
    deadline: Instant,
    timeout: Duration,
) -> AppResult<String> {
    loop {
        if let Some(result) = promise.result::<String>() {
            return result.map_err(|err| AppError::Extension(err.to_string()));
        }
        if Instant::now() >= deadline {
            return Err(AppError::ExtensionTimeout(timeout.as_secs()));
        }
        if !ctx.execute_pending_job() {
            let ran_timer: bool = ctx
                .eval(
                    "typeof globalThis.__kaminari_run_timers === 'function' \
                     ? globalThis.__kaminari_run_timers() > 0 : false",
                )
                .unwrap_or(false);
            if !ran_timer {
                return Err(AppError::Extension(
                    "extension did not finish: it is waiting on an unsupported async operation"
                        .to_string(),
                ));
            }
        }
    }
}

fn inject_bridge(ctx: &Ctx<'_>, host: Arc<HostContext>) -> AppResult<()> {
    let bridge = Object::new(ctx.clone()).map_err(|err| AppError::Extension(err.to_string()))?;

    let fetch_host = host.clone();
    let fetch = Function::new(ctx.clone(), move |url: String, options: String| {
        fetch_host.bridge_fetch(&url, &options)
    })
    .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("fetch", fetch)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    let select_host = host.clone();
    let select = Function::new(ctx.clone(), move |html: String, selector: String| {
        select_host.bridge_select(&html, &selector)
    })
    .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("select", select)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    let attr_host = host.clone();
    let attr = Function::new(
        ctx.clone(),
        move |html: String, selector: String, name: String| {
            attr_host.bridge_attr(&html, &selector, &name)
        },
    )
    .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("attr", attr)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    let sha_host = host.clone();
    let sha = Function::new(ctx.clone(), move |text: String| sha_host.sha256(&text))
        .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("sha256", sha)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    let random = Function::new(ctx.clone(), |count: u32| {
        let mut bytes = vec![0u8; (count as usize).min(4096)];
        match getrandom::fill(&mut bytes) {
            Ok(()) => json!({ "ok": true, "value": bytes }).to_string(),
            Err(err) => {
                json!({ "ok": false, "code": "RANDOM", "message": err.to_string() }).to_string()
            }
        }
    })
    .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("randomBytes", random)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    let get_host = host.clone();
    let storage_get = Function::new(ctx.clone(), move |key: String| get_host.storage_get(&key))
        .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("storageGet", storage_get)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    let set_host = host.clone();
    let storage_set = Function::new(ctx.clone(), move |key: String, value: String| {
        set_host.storage_set(&key, &value)
    })
    .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("storageSet", storage_set)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    let remove_host = host.clone();
    let storage_remove = Function::new(ctx.clone(), move |key: String| {
        remove_host.storage_remove(&key)
    })
    .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("storageRemove", storage_remove)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    let log_host = host.clone();
    let log = Function::new(ctx.clone(), move |level: String, message: String| {
        log_host.log(&level, &message)
    })
    .map_err(|err| AppError::Extension(err.to_string()))?;
    bridge
        .set("log", log)
        .map_err(|err| AppError::Extension(err.to_string()))?;

    ctx.globals()
        .set("__kaminari_host", bridge)
        .map_err(|err| AppError::Extension(err.to_string()))?;
    Ok(())
}

impl HostContext {
    fn bridge_fetch(&self, url: &str, options: &str) -> String {
        match self.fetch(url, options) {
            Ok(value) => json!({ "ok": true, "value": value }).to_string(),
            Err(err) => err.json(),
        }
    }

    fn fetch(&self, raw_url: &str, raw_options: &str) -> Result<Value, HostError> {
        let url = reqwest::Url::parse(raw_url)
            .map_err(|_| HostError::new("BAD_URL", "URL is not valid"))?;
        if url.scheme() != "https" {
            return Err(HostError::new("HOST_DENIED", "only https is allowed"));
        }
        if !self.allow.iter().any(|rule| rule.allows(&url)) {
            return Err(HostError::new(
                "HOST_DENIED",
                format!(
                    "host is not in the manifest allowlist: {}",
                    url.host_str().unwrap_or("")
                ),
            ));
        }
        if is_private_host(url.host_str().unwrap_or("")) {
            return Err(HostError::new(
                "HOST_DENIED",
                "localhost and private hosts are blocked",
            ));
        }

        let options: FetchOptions = serde_json::from_str(raw_options).unwrap_or(FetchOptions {
            method: None,
            headers: None,
            body: None,
            timeout_ms: None,
        });
        let method = match options
            .method
            .unwrap_or_else(|| "GET".to_string())
            .to_ascii_uppercase()
            .as_str()
        {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            _ => {
                return Err(HostError::new(
                    "BAD_OPTIONS",
                    "only GET and POST are allowed",
                ))
            }
        };

        let allow = self.allow.clone();
        let redirect_policy = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 3 {
                return attempt.error(std::io::Error::other("too many redirects"));
            }
            if allow.iter().any(|rule| rule.allows(attempt.url())) {
                attempt.follow()
            } else {
                attempt.error(std::io::Error::other("redirect host is not allowlisted"))
            }
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(
                options
                    .timeout_ms
                    .map(Duration::from_millis)
                    .unwrap_or(self.fetch_timeout)
                    .min(Duration::from_secs(MAX_FETCH_TIMEOUT_SECS)),
            )
            .redirect(redirect_policy)
            .build()
            .map_err(|err| HostError::new("NETWORK", err.to_string()))?;

        let mut request = client.request(method, url);
        if let Some(headers) = options.headers {
            for (name, value) in headers {
                request = request.header(name, value);
            }
        }
        if let Some(body) = options.body {
            request = request.body(body);
        }

        let response = request
            .send()
            .map_err(|err| HostError::new("NETWORK", err.to_string()))?;
        let status = response.status();
        let headers: Map<String, Value> = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.to_string(), Value::String(value.to_string())))
            })
            .collect();

        let mut body = String::new();
        response
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_string(&mut body)
            .map_err(|err| HostError::new("NETWORK", err.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(HostError::new(
                "RESPONSE_TOO_LARGE",
                "response is larger than 5 MB",
            ));
        }

        Ok(json!({
            "status": status.as_u16(),
            "headers": headers,
            "body": body,
        }))
    }

    fn bridge_select(&self, html: &str, selector: &str) -> String {
        match select(html, selector, self.deadline) {
            Ok(rows) => json!({ "ok": true, "value": rows }).to_string(),
            Err(err) => err.json(),
        }
    }

    fn bridge_attr(&self, html: &str, selector: &str, name: &str) -> String {
        match attr(html, selector, name) {
            Ok(value) => json!({ "ok": true, "value": value }).to_string(),
            Err(err) => err.json(),
        }
    }

    fn sha256(&self, text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn storage_get(&self, key: &str) -> String {
        match self.storage.get(&self.extension_id, key) {
            Some(value) => json!({ "found": true, "value": value }).to_string(),
            None => json!({ "found": false, "value": null }).to_string(),
        }
    }

    fn storage_set(&self, key: &str, value: &str) -> String {
        match self.storage.set(&self.extension_id, key, value) {
            Ok(()) => json!({ "ok": true }).to_string(),
            Err(err) => {
                json!({ "ok": false, "code": "STORAGE", "message": err.to_string() }).to_string()
            }
        }
    }

    fn storage_remove(&self, key: &str) -> String {
        match self.storage.remove(&self.extension_id, key) {
            Ok(()) => json!({ "ok": true }).to_string(),
            Err(err) => {
                json!({ "ok": false, "code": "STORAGE", "message": err.to_string() }).to_string()
            }
        }
    }

    fn log(&self, level: &str, message: &str) {
        match level {
            "error" => tracing::error!(extension = %self.extension_id, "{message}"),
            "warn" => tracing::warn!(extension = %self.extension_id, "{message}"),
            _ => tracing::info!(extension = %self.extension_id, "{message}"),
        }
    }
}

fn select(html: &str, selector: &str, deadline: Instant) -> Result<Value, HostError> {
    if Instant::now() >= deadline {
        return Err(select_deadline_error());
    }
    let document = Html::parse_document(html);
    if Instant::now() >= deadline {
        return Err(select_deadline_error());
    }
    let selector = Selector::parse(selector)
        .map_err(|_| HostError::new("BAD_SELECTOR", "selector is not valid"))?;

    let mut rows = Vec::new();
    // Account for the JSON array brackets as well as one separator per row.
    let mut output_bytes = 2usize;
    for element in document.select(&selector) {
        if Instant::now() >= deadline {
            return Err(select_deadline_error());
        }
        if rows.len() >= MAX_SELECT_NODES {
            break;
        }
        let remaining = MAX_SELECT_OUTPUT_BYTES.saturating_sub(output_bytes);
        if remaining <= SELECT_ROW_OVERHEAD_BYTES {
            break;
        }
        let mut content_budget = remaining - SELECT_ROW_OVERHEAD_BYTES;
        loop {
            let row = build_select_row(element, content_budget);
            if Instant::now() >= deadline {
                return Err(select_deadline_error());
            }
            let encoded_len = serde_json::to_string(&row)
                .map_err(|err| HostError::new("HTML_SELECT", err.to_string()))?
                .len();
            if output_bytes + encoded_len < MAX_SELECT_OUTPUT_BYTES {
                output_bytes += encoded_len + 1;
                rows.push(row);
                break;
            }
            if content_budget == 0 {
                break;
            }
            // A single huge node should still produce a useful, smaller row.
            content_budget /= 2;
        }
        if rows.len() >= MAX_SELECT_NODES {
            break;
        }
    }
    Ok(Value::Array(rows))
}

fn build_select_row(element: scraper::ElementRef<'_>, budget: usize) -> Value {
    let mut remaining = budget;

    let text = collect_text(element, MAX_SELECT_TEXT_BYTES.min(remaining));
    remaining -= text.len();

    let html_budget = MAX_SELECT_HTML_BYTES.min(remaining);
    let html = if html_budget == 0 {
        String::new()
    } else {
        truncate_utf8(&element.html(), html_budget).to_string()
    };
    remaining -= html.len();

    let mut attrs = Map::new();
    if remaining > 0 {
        for (name, value) in element.value().attrs().take(MAX_SELECT_ATTRS) {
            if remaining == 0 {
                break;
            }
            let name = truncate_utf8(name, MAX_SELECT_ATTR_NAME_BYTES.min(remaining));
            remaining -= name.len();
            if remaining == 0 {
                attrs.insert(name.to_string(), Value::String(String::new()));
                break;
            }
            let value = truncate_utf8(value, remaining);
            remaining -= value.len();
            attrs.insert(name.to_string(), Value::String(value.to_string()));
        }
    }

    json!({
        "text": text,
        "html": html,
        "attrs": attrs,
    })
}

fn collect_text(element: scraper::ElementRef<'_>, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    let mut text = String::new();
    for part in element.text() {
        let remaining = max_bytes.saturating_sub(text.len());
        if remaining == 0 {
            break;
        }
        if part.len() <= remaining {
            text.push_str(part);
        } else {
            text.push_str(truncate_utf8(part, remaining));
            break;
        }
    }
    text
}

fn truncate_utf8(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn select_deadline_error() -> HostError {
    HostError::new("DEADLINE", "html.select exceeded the extension time limit")
}

fn attr(html: &str, selector: &str, name: &str) -> Result<Option<String>, HostError> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(selector)
        .map_err(|_| HostError::new("BAD_SELECTOR", "selector is not valid"))?;
    Ok(document
        .select(&selector)
        .next()
        .and_then(|element| element.value().attr(name))
        .map(|value| value.to_string()))
}

pub(crate) fn is_private_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified()
        }
        Ok(IpAddr::V6(ip)) => ip.is_loopback() || ip.is_unspecified(),
        Err(_) => false,
    }
}

fn invalid_manifest(message: &str) -> AppError {
    AppError::Extension(format!("invalid manifest: {message}"))
}

const BRIDGE_SHIM: &str = r#"
(function () {
  const host = globalThis.__kaminari_host;
  const parse = (raw) => JSON.parse(raw);
  const fail = (out) => {
    const error = new Error((out.code || "HOST_ERROR") + ": " + (out.message || "host error"));
    error.code = out.code || "HOST_ERROR";
    throw error;
  };
  const call = (raw) => {
    const out = parse(raw);
    if (!out.ok) fail(out);
    return out.value;
  };

  globalThis.net = {
    fetch(url, options) {
      return call(host.fetch(String(url), JSON.stringify(options || {})));
    }
  };

  globalThis.html = {
    select(html, selector) {
      return call(host.select(String(html), String(selector)));
    },
    attr(html, selector, name) {
      return call(host.attr(String(html), String(selector), String(name)));
    }
  };

  globalThis.crypto = {
    sha256(text) {
      return host.sha256(String(text));
    }
  };

  globalThis.random = {
    bytes(count) {
      return call(host.randomBytes(Number(count) || 0));
    }
  };

  globalThis.storage = {
    get(key) {
      const out = parse(host.storageGet(String(key)));
      return out.found ? out.value : null;
    },
    set(key, value) {
      const out = parse(host.storageSet(String(key), String(value)));
      if (!out.ok) fail(out);
    },
    remove(key) {
      const out = parse(host.storageRemove(String(key)));
      if (!out.ok) fail(out);
    }
  };

  globalThis.logger = {
    info(message) { host.log("info", String(message)); },
    warn(message) { host.log("warn", String(message)); },
    error(message) { host.log("error", String(message)); }
  };
})();
"#;

const DISPATCH: &str = r#"
(async function () {
  try {
    const op = globalThis.__kaminari_op;
    const provider = globalThis.kaminari;
    const fn = provider && provider[op];
    if (typeof fn !== "function") {
      const error = new Error("Missing method: " + op);
      error.code = "MISSING_METHOD";
      throw error;
    }
    const args = JSON.parse(globalThis.__kaminari_args);
    const out = await fn.apply(null, args);
    const value = JSON.stringify(out);
    if (value === undefined) {
      throw new Error("Extension returned no value");
    }
    return JSON.stringify({ ok: true, value: JSON.parse(value) });
  } catch (error) {
    return JSON.stringify({
      ok: false,
      code: error && error.code ? String(error.code) : "EXTENSION_ERROR",
      message: error && error.message ? String(error.message) : String(error)
    });
  }
})()
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn package(code: &str) -> ExtPackage {
        ExtPackage {
            manifest: Manifest {
                id: "app.kaminari.test".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                api_version: "0.1".to_string(),
                entry: "index.js".to_string(),
                lang: "en".to_string(),
                base_url: "https://example.com".to_string(),
                min_host: "0.1.0".to_string(),
                nsfw: false,
                icon: None,
                permissions: Permissions {
                    hosts: vec!["https://archive.org/*".to_string()],
                    storage_kb: Some(512),
                },
            },
            code: code.to_string(),
        }
    }

    fn limits(timeout_ms: u64, memory_bytes: usize) -> HostLimits {
        HostLimits {
            timeout: Duration::from_millis(timeout_ms),
            resolve_timeout: Duration::from_millis(timeout_ms),
            memory_bytes,
            stack_bytes: 512 * 1024,
        }
    }

    fn storage() -> Arc<dyn ExtStorage> {
        Arc::new(MemoryStorage::default())
    }

    #[test]
    fn legacy_source_ids_stay_digits_only() {
        assert!(validate_source_id("8542735178285060053"));
        assert!(!validate_source_id("abc"));
        assert!(!validate_source_id(""));
    }

    #[test]
    fn runs_a_static_provider() {
        let code = r#"
          globalThis.kaminari = {
            search(query, page) {
              return { items: [{ id: "1", title: "Test " + query, year: 2026 }], hasNext: false };
            }
          };
        "#;
        let out = run(
            &package(code),
            "search",
            json!(["hello", 1]),
            storage(),
            &limits(1000, 8 * 1024 * 1024),
        )
        .unwrap();
        assert_eq!(out["items"][0]["title"], "Test hello");
        assert_eq!(out["items"][0]["year"], 2026);
    }

    #[test]
    fn missing_method_is_an_error() {
        let code = "globalThis.kaminari = {};";
        let err = run(
            &package(code),
            "search",
            json!(["hello", 1]),
            storage(),
            &limits(1000, 8 * 1024 * 1024),
        )
        .unwrap_err();
        assert!(err.to_string().contains("MISSING_METHOD"));
    }

    #[test]
    fn async_methods_resolve_through_the_microtask_queue() {
        let code = r#"
          globalThis.kaminari = {
            async search(query) {
              await Promise.resolve();
              const second = await Promise.resolve(query + "!");
              return { items: [{ id: "1", title: second }], hasNext: false };
            }
          };
        "#;
        let out = run(
            &package(code),
            "search",
            json!(["hello", 1]),
            storage(),
            &limits(1000, 8 * 1024 * 1024),
        )
        .unwrap();
        assert_eq!(out["items"][0]["title"], "hello!");
    }

    #[test]
    fn wildcard_hosts_allow_any_https_host() {
        let rule = HostRule::parse("*").unwrap();
        assert!(rule.allows(&reqwest::Url::parse("https://cdn.example/a.m3u8").unwrap()));
        assert!(!rule.allows(&reqwest::Url::parse("http://cdn.example/a.m3u8").unwrap()));
    }

    #[test]
    fn infinite_loop_hits_the_timeout() {
        let code = "globalThis.kaminari = { search: () => { while (true) {} } };";
        let err = run(
            &package(code),
            "search",
            json!(["hello", 1]),
            storage(),
            &limits(100, 8 * 1024 * 1024),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::ExtensionTimeout(_)));
    }

    #[test]
    fn allocation_is_stopped_by_the_memory_limit() {
        let code = r#"
          globalThis.kaminari = {
            search() {
              const rows = [];
              while (true) rows.push("x".repeat(1024 * 1024));
            }
          };
        "#;
        let err = run(
            &package(code),
            "search",
            json!(["hello", 1]),
            storage(),
            &limits(2000, 2 * 1024 * 1024),
        )
        .unwrap_err();
        assert!(
            matches!(err, AppError::ExtensionTimeout(_))
                || err.to_string().to_lowercase().contains("memory")
        );
    }

    #[test]
    fn host_denial_happens_before_the_request() {
        let code = r#"
          globalThis.kaminari = {
            search() { return net.fetch("https://example.com/secret"); }
          };
        "#;
        let err = run(
            &package(code),
            "search",
            json!(["hello", 1]),
            storage(),
            &limits(1000, 8 * 1024 * 1024),
        )
        .unwrap_err();
        assert!(err.to_string().contains("HOST_DENIED"));
    }

    #[test]
    fn storage_survives_a_fresh_runtime() {
        let write = r#"
          globalThis.kaminari = {
            search() { storage.set("token", "abc"); return { value: storage.get("token") }; }
          };
        "#;
        let storage = storage();
        let first = run(
            &package(write),
            "search",
            json!(["hello", 1]),
            storage.clone(),
            &limits(1000, 8 * 1024 * 1024),
        )
        .unwrap();
        assert_eq!(first["value"], "abc");

        let read = r#"
          globalThis.kaminari = {
            search() { return { value: storage.get("token") }; }
          };
        "#;
        let second = run(
            &package(read),
            "search",
            json!(["hello", 1]),
            storage,
            &limits(1000, 8 * 1024 * 1024),
        )
        .unwrap();
        assert_eq!(second["value"], "abc");
    }

    #[test]
    fn html_select_returns_text_and_attrs() {
        let code = r#"
          globalThis.kaminari = {
            search() {
              const rows = html.select('<div class="item" data-id="7">Hello</div>', ".item");
              return { text: rows[0].text, id: rows[0].attrs["data-id"] };
            }
          };
        "#;
        let out = run(
            &package(code),
            "search",
            json!(["hello", 1]),
            storage(),
            &limits(1000, 8 * 1024 * 1024),
        )
        .unwrap();
        assert_eq!(out["text"], "Hello");
        assert_eq!(out["id"], "7");
    }

    #[test]
    fn html_select_caps_generated_nested_documents() {
        let depth = 3000;
        let mut html = String::with_capacity(depth * 1750);
        html.push_str("<!doctype html><html><body>");
        for index in 0..depth {
            html.push_str("<div data-index=\"");
            html.push_str(&index.to_string());
            html.push_str("\">");
            for _ in 0..70 {
                html.push_str("日本語のテキスト");
            }
        }
        for _ in 0..depth {
            html.push_str("</div>");
        }
        html.push_str("</body></html>");
        assert!(html.len() > 5_000_000);

        let rows = select(&html, "*", Instant::now() + Duration::from_secs(10)).unwrap();
        let rows = rows.as_array().unwrap();
        assert!(!rows.is_empty());
        assert!(rows.len() <= MAX_SELECT_NODES);
        let encoded = serde_json::to_string(&Value::Array(rows.clone())).unwrap();
        assert!(encoded.len() <= MAX_SELECT_OUTPUT_BYTES);
        for row in rows {
            assert!(row["text"].as_str().unwrap().len() <= MAX_SELECT_TEXT_BYTES);
            assert!(row["html"].as_str().unwrap().len() <= MAX_SELECT_HTML_BYTES);
            assert!(row["attrs"].as_object().unwrap().len() <= MAX_SELECT_ATTRS);
        }
    }

    #[test]
    fn html_select_caps_node_count_and_attributes() {
        let mut html = String::from("<html><body>");
        for index in 0..(MAX_SELECT_NODES + 25) {
            html.push_str("<p class=\"item\"");
            for attr in 0..(MAX_SELECT_ATTRS + 8) {
                html.push_str(&format!(" data-{attr}=\"v\""));
            }
            html.push_str(">row ");
            html.push_str(&index.to_string());
            html.push_str("</p>");
        }
        html.push_str("</body></html>");

        let rows = select(&html, ".item", Instant::now() + Duration::from_secs(10)).unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), MAX_SELECT_NODES);
        for row in rows {
            assert_eq!(row["attrs"].as_object().unwrap().len(), MAX_SELECT_ATTRS);
        }
    }

    #[test]
    fn html_select_honors_the_deadline() {
        let err = select(
            "<div>Hello</div>",
            "div",
            Instant::now() - Duration::from_millis(1),
        )
        .unwrap_err();
        assert_eq!(err.code, "DEADLINE");
    }

    #[test]
    fn utf8_truncation_never_splits_characters() {
        let text = "日".repeat(10);
        let truncated = truncate_utf8(&text, 8);
        assert_eq!(truncated, "日日");
        assert_eq!(truncate_utf8("short", 100), "short");
        assert_eq!(truncate_utf8(&text, 0), "");
    }

    #[test]
    fn kext_round_trip_and_sample() {
        use std::io::Write;

        let sample = sample_package().unwrap();
        let manifest_raw = serde_json::to_string(&sample.manifest).unwrap();
        let mut bytes = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("manifest.json", options).unwrap();
            writer.write_all(manifest_raw.as_bytes()).unwrap();
            writer.start_file("index.js", options).unwrap();
            writer.write_all(sample.code.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        let parsed = package_from_kext(&bytes).unwrap();
        assert_eq!(parsed.manifest.id, "app.kaminari.publicdomain");
        assert!(parsed.code.contains("globalThis.kaminari"));
        assert!(package_from_kext(b"not a zip").is_err());
    }
}
