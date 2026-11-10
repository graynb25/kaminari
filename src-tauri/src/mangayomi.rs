// Mangayomi compatibility layer. Third party JavaScript is wrapped with a
// bootstrap that implements the subset of the Mangayomi provider API that
// anime sources use. The wrapper is stored as a normal extension and exposes
// the four native operations through globalThis.kaminari.

use std::time::{Duration, Instant};

use rquickjs::{Context, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::ext_host::{self, Manifest, Permissions};

pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
const PREVIEW_TIMEOUT_SECS: u64 = 5;

const PREVIEW_STUBS: &str = r##"
(function () {
  globalThis.net = {
    fetch() { throw new Error("network is disabled during inspection"); }
  };
  globalThis.html = {
    select() { return []; },
    attr() { return null; }
  };
  globalThis.storage = {
    get() { return null; },
    set() {},
    remove() {}
  };
  globalThis.random = {
    bytes(count) { return new Array(Number(count) || 0).fill(0); }
  };
  globalThis.crypto = {
    sha256() { return ""; }
  };
  globalThis.logger = { info() {}, warn() {}, error() {} };
})();
"##;

const BOOTSTRAP: &str = r##"
(function () {
  const log = globalThis.logger || { info() {}, warn() {}, error() {} };

  class KElement {
    constructor(data) {
      this._text = data && data.text ? String(data.text) : "";
      this._html = data && data.html ? String(data.html) : "";
      this._attrs = (data && data.attrs) || {};
    }
    get text() { return this._text; }
    get html() { return this._html; }
    attr(name) {
      const value = this._attrs[String(name)];
      return value == null ? null : String(value);
    }
    select(selector) {
      const rows = globalThis.html && typeof globalThis.html.select === "function"
        ? globalThis.html.select(this._html, String(selector))
        : [];
      return rows.map((row) => new KElement(row));
    }
    selectFirst(selector) {
      const rows = this.select(selector);
      return rows.length ? rows[0] : null;
    }
    toString() { return this._text; }
  }

  class KDocument {
    constructor(html) { this._html = String(html == null ? "" : html); }
    select(selector) {
      const rows = globalThis.html && typeof globalThis.html.select === "function"
        ? globalThis.html.select(this._html, String(selector))
        : [];
      return rows.map((row) => new KElement(row));
    }
    selectFirst(selector) {
      const rows = this.select(selector);
      return rows.length ? rows[0] : null;
    }
  }

  globalThis.Document = KDocument;
  globalThis.Element = KElement;

  class SharedPreferences {
    get(key) { return globalThis.storage.get("pref." + String(key)); }
    set(key, value) { globalThis.storage.set("pref." + String(key), String(value)); }
    remove(key) { globalThis.storage.remove("pref." + String(key)); }
  }
  globalThis.SharedPreferences = SharedPreferences;

  class Client {
    constructor(options) { this.options = options || {}; }
    async get(url, headers) { return this._request("GET", url, headers, null); }
    async post(url, headers, body) { return this._request("POST", url, headers, body); }
    async _request(method, url, headers, body) {
      const options = { method: method, headers: headers || {} };
      if (body !== null && body !== undefined) {
        options.body = typeof body === "string" ? body : JSON.stringify(body);
      }
      const res = globalThis.net.fetch(String(url), options);
      return {
        statusCode: res.status,
        body: res.body,
        headers: res.headers || {},
        url: String(url)
      };
    }
  }
  globalThis.Client = Client;

  class MProvider {
    constructor() { this.source = null; }
    get supportsLatest() { return false; }
  }
  globalThis.MProvider = MProvider;

  function logMessage(level, args) {
    const text = Array.prototype.map.call(args, (value) => {
      if (value instanceof Error) return value.message;
      if (typeof value === "object" && value !== null) {
        try { return JSON.stringify(value); } catch (e) { return String(value); }
      }
      return String(value);
    }).join(" ");
    if (typeof log[level] === "function") log[level](text);
  }
  globalThis.console = {
    log(...args) { logMessage("info", args); },
    info(...args) { logMessage("info", args); },
    warn(...args) { logMessage("warn", args); },
    error(...args) { logMessage("error", args); },
    debug(...args) { logMessage("info", args); }
  };

  const b64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  if (typeof globalThis.btoa !== "function") {
    globalThis.btoa = function (input) {
      let out = "";
      const str = String(input);
      for (let i = 0; i < str.length;) {
        const a = str.charCodeAt(i++);
        const b = i < str.length ? str.charCodeAt(i++) : NaN;
        const c = i < str.length ? str.charCodeAt(i++) : NaN;
        out += b64[a >> 2];
        out += b64[((a & 3) << 4) | ((b || 0) >> 4)];
        out += isNaN(b) ? "=" : b64[((b & 15) << 2) | ((c || 0) >> 6)];
        out += isNaN(c) ? "=" : b64[c & 63];
      }
      return out;
    };
  }
  if (typeof globalThis.atob !== "function") {
    globalThis.atob = function (input) {
      const str = String(input).replace(/[^A-Za-z0-9+/]/g, "");
      let out = "";
      let acc = 0;
      let bits = 0;
      for (let i = 0; i < str.length; i++) {
        acc = (acc << 6) | b64.indexOf(str[i]);
        bits += 6;
        if (bits >= 8) {
          bits -= 8;
          out += String.fromCharCode((acc >> bits) & 255);
          acc &= (1 << bits) - 1;
        }
      }
      return out;
    };
  }

  function utf8Encode(input) {
    const str = String(input);
    const out = [];
    for (let i = 0; i < str.length; i++) {
      let code = str.charCodeAt(i);
      if (code >= 0xd800 && code <= 0xdbff && i + 1 < str.length) {
        const next = str.charCodeAt(i + 1);
        if (next >= 0xdc00 && next <= 0xdfff) {
          code = 0x10000 + ((code - 0xd800) << 10) + (next - 0xdc00);
          i++;
        }
      }
      if (code < 0x80) out.push(code);
      else if (code < 0x800) out.push(0xc0 | (code >> 6), 0x80 | (code & 63));
      else if (code < 0x10000) out.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63));
      else out.push(0xf0 | (code >> 18), 0x80 | ((code >> 12) & 63), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63));
    }
    return new Uint8Array(out);
  }

  function utf8Decode(input) {
    const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
    let out = "";
    for (let i = 0; i < bytes.length;) {
      const first = bytes[i++];
      if (first < 0x80) out += String.fromCharCode(first);
      else if (first < 0xe0) out += String.fromCharCode(((first & 31) << 6) | (bytes[i++] & 63));
      else if (first < 0xf0) out += String.fromCharCode(((first & 15) << 12) | ((bytes[i++] & 63) << 6) | (bytes[i++] & 63));
      else {
        const code = ((first & 7) << 18) | ((bytes[i++] & 63) << 12) | ((bytes[i++] & 63) << 6) | (bytes[i++] & 63);
        const value = code - 0x10000;
        out += String.fromCharCode(0xd800 + (value >> 10), 0xdc00 + (value & 1023));
      }
    }
    return out;
  }

  if (typeof globalThis.TextEncoder !== "function") {
    globalThis.TextEncoder = class TextEncoder {
      encode(input) { return utf8Encode(input); }
      encodeInto(input, dest) {
        const bytes = utf8Encode(input);
        dest.set(bytes.subarray(0, dest.length));
        return { read: input.length, written: Math.min(bytes.length, dest.length) };
      }
    };
  }
  if (typeof globalThis.TextDecoder !== "function") {
    globalThis.TextDecoder = class TextDecoder {
      constructor(label) { this.encoding = label || "utf-8"; }
      decode(input) { return input ? utf8Decode(input) : ""; }
    };
  }

  globalThis.crypto = globalThis.crypto || {};
  if (typeof globalThis.crypto.getRandomValues !== "function" && globalThis.random) {
    globalThis.crypto.getRandomValues = function (array) {
      const values = globalThis.random.bytes(array.length);
      for (let i = 0; i < array.length; i++) array[i] = values[i] & 255;
      return array;
    };
  }

  let timerId = 1;
  globalThis.__kaminari_timers = [];
  globalThis.setTimeout = function (fn) {
    const id = timerId++;
    globalThis.__kaminari_timers.push({
      id: id,
      fn: fn,
      args: Array.prototype.slice.call(arguments, 2)
    });
    return id;
  };
  globalThis.clearTimeout = function (id) {
    globalThis.__kaminari_timers = globalThis.__kaminari_timers.filter((t) => t.id !== id);
  };
  globalThis.setInterval = function () { return timerId++; };
  globalThis.clearInterval = function () {};
  globalThis.queueMicrotask = function (fn) { Promise.resolve().then(fn); };
  globalThis.__kaminari_run_timers = function () {
    const pending = globalThis.__kaminari_timers;
    globalThis.__kaminari_timers = [];
    for (const timer of pending) {
      try { timer.fn.apply(null, timer.args); } catch (e) { logMessage("error", [e]); }
    }
    return pending.length;
  };

  class KURLSearchParams {
    constructor(init) {
      this._pairs = [];
      if (typeof init === "string") {
        String(init).replace(/^\?/, "").split("&").forEach((part) => {
          if (!part) return;
          const index = part.indexOf("=");
          const key = index < 0 ? part : part.slice(0, index);
          const value = index < 0 ? "" : part.slice(index + 1);
          this._pairs.push([decodeURIComponent(key), decodeURIComponent(value)]);
        });
      }
    }
    get(key) {
      for (const pair of this._pairs) if (pair[0] === key) return pair[1];
      return null;
    }
    has(key) { return this.get(key) !== null; }
    append(key, value) { this._pairs.push([String(key), String(value)]); }
    set(key, value) {
      let found = false;
      this._pairs = this._pairs.filter((pair) => {
        if (pair[0] !== key) return true;
        if (!found) { pair[1] = String(value); found = true; return true; }
        return false;
      });
      if (!found) this.append(key, value);
    }
    toString() {
      return this._pairs.map((pair) => encodeURIComponent(pair[0]) + "=" + encodeURIComponent(pair[1])).join("&");
    }
  }
  globalThis.URLSearchParams = globalThis.URLSearchParams || KURLSearchParams;

  class KURL {
    constructor(input, base) {
      let raw = String(input);
      if (!/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(raw)) {
        const baseRaw = String(base || "https://example.invalid/");
        const baseMatch = baseRaw.match(/^([a-zA-Z][a-zA-Z0-9+.-]*:)\/\/([^/?#]+)([^?#]*)/);
        if (!baseMatch) throw new Error("Invalid URL");
        if (raw.startsWith("//")) raw = baseMatch[1] + raw;
        else if (raw.startsWith("/")) raw = baseMatch[1] + "//" + baseMatch[2] + raw;
        else {
          const dir = baseMatch[3].replace(/[^/]*$/, "");
          raw = baseMatch[1] + "//" + baseMatch[2] + dir + raw;
        }
      }
      const match = raw.match(/^([a-zA-Z][a-zA-Z0-9+.-]*:)\/\/([^/?#]*)([^?#]*)(\?[^#]*)?(#.*)?$/);
      if (!match) throw new Error("Invalid URL");
      this.protocol = match[1];
      this.host = match[2];
      this.hostname = this.host.replace(/:\d+$/, "");
      this.port = (this.host.match(/:(\d+)$/) || [])[1] || "";
      this.pathname = match[3] || "/";
      this.search = match[4] || "";
      this.hash = match[5] || "";
      this.origin = this.protocol + "//" + this.host;
      this.href = this.origin + this.pathname + this.search + this.hash;
      this.searchParams = new globalThis.URLSearchParams(this.search);
    }
    toString() { return this.href; }
  }
  globalThis.URL = globalThis.URL || KURL;

  globalThis.window = globalThis;
  globalThis.navigator = globalThis.navigator || {
    userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36"
  };
  globalThis.location = globalThis.location || {
    href: "https://example.invalid/",
    origin: "https://example.invalid",
    protocol: "https:",
    host: "example.invalid",
    hostname: "example.invalid",
    pathname: "/",
    search: "",
    hash: ""
  };
})();
"##;

const ADAPTER_TEMPLATE: &str = r##"
;(function () {
  const sourceIndex = __SOURCE_INDEX__;
  const sources = typeof mangayomiSources === "undefined" ? null : mangayomiSources;
  const source = sources && sources[sourceIndex];
  if (!source) throw new Error("Mangayomi source " + sourceIndex + " is missing");
  if (typeof DefaultExtension !== "function") {
    throw new Error("DefaultExtension is missing");
  }

  const base = String(source.baseUrl || "").replace(/\/$/, "");
  globalThis.location = {
    href: base + "/",
    origin: base,
    protocol: "https:",
    host: base.replace(/^https?:\/\//, ""),
    hostname: base.replace(/^https?:\/\//, ""),
    pathname: "/",
    search: "",
    hash: ""
  };

  function absolute(url) {
    const raw = String(url || "");
    if (/^https?:\/\//.test(raw)) return raw;
    if (!raw) return "";
    return base + "/" + raw.replace(/^\//, "");
  }

  function episodeNumber(name, index) {
    const text = String(name || "");
    let match = text.match(/(?:^|\b)(?:e|ep|episode)\s*([0-9]+(?:\.[0-9]+)?)/i);
    if (!match) match = text.match(/([0-9]+(?:\.[0-9]+)?)/);
    return match ? Number(match[1]) : index + 1;
  }

  function statusName(status) {
    const code = Number(status);
    if (code === 0) return "airing";
    if (code === 1) return "finished";
    if (code === 4) return "upcoming";
    return "unknown";
  }

  function streamType(url) {
    if (/^edl:\/\//i.test(url)) return "edl";
    const path = String(url).split(/[?#]/)[0].toLowerCase();
    if (path.endsWith(".m3u8")) return "hls";
    if (path.endsWith(".mp4")) return "mp4";
    if (path.endsWith(".mpd")) return "dash";
    if (path.endsWith(".mkv")) return "mkv";
    return "hls";
  }

  function toVtt(text) {
    const body = String(text).replace(/\r\n/g, "\n").replace(/\r/g, "\n");
    if (/^WEBVTT/.test(body)) return body;
    return "WEBVTT\n\n" + body
      .replace(/^\d+\s*$/gm, "")
      .replace(/(\d{2}:\d{2}:\d{2}),(\d{3})/g, "$1.$2");
  }

  function subtitleList(list) {
    if (!Array.isArray(list)) return null;
    const out = [];
    for (const sub of list) {
      if (!sub) continue;
      const raw = typeof sub === "string" ? sub : (sub.file || sub.url || "");
      if (!raw) continue;
      let url = "";
      if (/^https?:\/\//.test(raw)) url = raw;
      else if (typeof sub === "object" && /-->/.test(raw)) {
        url = "data:text/vtt;charset=utf-8," + encodeURIComponent(toVtt(raw));
      }
      if (!url) continue;
      out.push({
        lang: String(sub.lang || sub.label || "sub"),
        url: url,
        label: sub.label ? String(sub.label) : undefined
      });
    }
    return out.length ? out : null;
  }

  function headerList(headers) {
    if (!headers || typeof headers !== "object") return null;
    const out = {};
    for (const name of Object.keys(headers)) {
      const value = headers[name];
      if (typeof value === "string") out[name] = value;
    }
    return Object.keys(out).length ? out : null;
  }

  const provider = new DefaultExtension();
  provider.source = source;

  globalThis.kaminari = {
    async search(query, page) {
      const out = await provider.search(String(query || ""), Number(page) || 1, []);
      const list = out && Array.isArray(out.list) ? out.list : [];
      return {
        items: list
          .filter((item) => item && item.name)
          .map((item) => ({
            id: absolute(item.link),
            title: String(item.name),
            cover: item.imageUrl ? String(item.imageUrl) : null
          }))
          .filter((item) => item.id),
        hasNext: !!(out && out.hasNextPage)
      };
    },

    async getShow(showId) {
      const detail = await provider.getDetail(String(showId));
      const genres = detail && (detail.genre || detail.genres);
      return {
        id: String(showId),
        title: String((detail && detail.name) || "Untitled"),
        cover: (detail && detail.imageUrl) || null,
        banner: (detail && (detail.banner || detail.bannerUrl)) || null,
        synopsis: (detail && detail.description) || null,
        year: null,
        genres: Array.isArray(genres) ? genres.filter(Boolean).map(String) : [],
        status: statusName(detail && detail.status)
      };
    },

    async getEpisodes(showId) {
      const detail = await provider.getDetail(String(showId));
      const chapters = detail && (
        Array.isArray(detail.chapters) ? detail.chapters :
        (Array.isArray(detail.episodes) ? detail.episodes : [])
      );
      return chapters.map((chapter, index) => ({
        id: String(chapter.url || chapter.link || (String(showId) + "#" + (index + 1))),
        showId: String(showId),
        number: episodeNumber(chapter.name || chapter.title, index),
        title: String(chapter.name || chapter.title || ("Episode " + (index + 1))),
        thumbnail: chapter.thumbnailUrl || chapter.thumbnail || null,
        airedAt: chapter.dateUpload || chapter.airedAt || null
      })).sort((a, b) => a.number - b.number);
    },

    async resolveEpisode(episodeId) {
      const list = await provider.getVideoList(String(episodeId));
      const streams = Array.isArray(list) ? list : [];
      return streams
        .map((stream) => {
          const url = String((stream && (stream.url || stream.originalUrl)) || "");
          return {
            url: url,
            type: streamType(url),
            quality: (stream && stream.quality) || null,
            headers: headerList(stream && stream.headers),
            subtitles: subtitleList(stream && stream.subtitles)
          };
        })
        .filter((stream) => /^https?:\/\//.test(stream.url) || stream.url.startsWith("edl://"));
    }
  };
})();
"##;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangayomiSource {
    pub name: String,
    #[serde(default)]
    pub id: Value,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub is_manga: bool,
    #[serde(default)]
    pub item_type: Option<i32>,
    #[serde(default)]
    pub source_code_url: Option<String>,
    #[serde(default)]
    pub api_url: Option<String>,
    #[serde(default)]
    pub is_nsfw: bool,
}

impl MangayomiSource {
    pub fn is_anime(&self) -> bool {
        !self.is_manga && self.item_type != Some(0)
    }

    fn identity(&self) -> String {
        match &self.id {
            Value::String(value) => value.clone(),
            Value::Number(value) => value.to_string(),
            _ => self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSource {
    pub index: usize,
    pub name: String,
    pub version: String,
    pub lang: String,
    pub base_url: String,
    pub source_code_url: Option<String>,
    pub nsfw: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub sources: Vec<PreviewSource>,
    pub has_provider: bool,
}

pub fn preview(code: &str) -> AppResult<Preview> {
    if code.len() > MAX_SOURCE_BYTES {
        return Err(AppError::Extension(
            "source is larger than 2 MB".to_string(),
        ));
    }
    let runtime = Runtime::new().map_err(|err| AppError::Extension(err.to_string()))?;
    runtime.set_memory_limit(ext_host::MEMORY_BYTES);
    runtime.set_max_stack_size(ext_host::STACK_BYTES);
    let deadline = Instant::now() + Duration::from_secs(PREVIEW_TIMEOUT_SECS);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));

    let context = Context::full(&runtime).map_err(|err| AppError::Extension(err.to_string()))?;
    context.with(|ctx| {
        ctx.eval::<(), _>(PREVIEW_STUBS)
            .map_err(|err| AppError::Extension(format!("inspection failed: {err}")))?;
        ctx.eval::<(), _>(BOOTSTRAP)
            .map_err(|err| AppError::Extension(format!("inspection failed: {err}")))?;
        ctx.eval::<(), _>(code)
            .map_err(|err| AppError::Extension(format!("source is not valid JavaScript: {err}")))?;
        let raw: String = ctx
            .eval(
                r#"JSON.stringify({
                    sources: typeof mangayomiSources === "undefined" ? null : mangayomiSources,
                    hasProvider: typeof DefaultExtension === "function"
                })"#,
            )
            .map_err(|err| AppError::Extension(format!("inspection failed: {err}")))?;
        let value: Value = serde_json::from_str(&raw).map_err(|err| {
            AppError::Extension(format!("inspection returned invalid JSON: {err}"))
        })?;
        let has_provider = value
            .get("hasProvider")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let Some(rows) = value.get("sources").and_then(Value::as_array) else {
            return Err(AppError::Extension(
                "mangayomiSources is missing. This is not a Mangayomi extension.".to_string(),
            ));
        };
        let mut sources = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let Ok(source) = serde_json::from_value::<MangayomiSource>(row.clone()) else {
                continue;
            };
            if !source.is_anime()
                || source.name.trim().is_empty()
                || source.base_url.trim().is_empty()
            {
                continue;
            }
            sources.push(PreviewSource {
                index,
                name: source.name,
                version: source.version,
                lang: source.lang,
                base_url: source.base_url,
                source_code_url: source.source_code_url,
                nsfw: source.is_nsfw,
            });
        }
        if sources.is_empty() {
            return Err(AppError::Extension(
                "the extension has no usable anime sources. Manga sources are not supported."
                    .to_string(),
            ));
        }
        if !has_provider {
            return Err(AppError::Extension(
                "DefaultExtension is missing. This is not a runnable Mangayomi source.".to_string(),
            ));
        }
        Ok(Preview {
            sources,
            has_provider,
        })
    })
}

pub fn source_at(code: &str, index: usize) -> AppResult<MangayomiSource> {
    let runtime = Runtime::new().map_err(|err| AppError::Extension(err.to_string()))?;
    runtime.set_memory_limit(ext_host::MEMORY_BYTES);
    runtime.set_max_stack_size(ext_host::STACK_BYTES);
    let deadline = Instant::now() + Duration::from_secs(PREVIEW_TIMEOUT_SECS);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let context = Context::full(&runtime).map_err(|err| AppError::Extension(err.to_string()))?;
    context.with(|ctx| {
        ctx.eval::<(), _>(PREVIEW_STUBS)
            .map_err(|err| AppError::Extension(format!("inspection failed: {err}")))?;
        ctx.eval::<(), _>(BOOTSTRAP)
            .map_err(|err| AppError::Extension(format!("inspection failed: {err}")))?;
        ctx.eval::<(), _>(code)
            .map_err(|err| AppError::Extension(format!("source is not valid JavaScript: {err}")))?;
        let raw: String = ctx
            .eval(
                r#"JSON.stringify(
                    typeof mangayomiSources === "undefined" ? null : mangayomiSources
                )"#,
            )
            .map_err(|err| AppError::Extension(format!("inspection failed: {err}")))?;
        let value: Value = serde_json::from_str(&raw).map_err(|err| {
            AppError::Extension(format!("inspection returned invalid JSON: {err}"))
        })?;
        let rows = value
            .as_array()
            .ok_or_else(|| AppError::Extension("mangayomiSources is missing".to_string()))?;
        let row = rows
            .get(index)
            .ok_or_else(|| AppError::Extension("source index is out of range".to_string()))?;
        let source: MangayomiSource = serde_json::from_value(row.clone())
            .map_err(|err| AppError::Extension(format!("source metadata is invalid: {err}")))?;
        if !source.is_anime() {
            return Err(AppError::Extension(
                "manga sources are not supported".to_string(),
            ));
        }
        if source.name.trim().is_empty() || source.base_url.trim().is_empty() {
            return Err(AppError::Extension(
                "source name and baseUrl are required".to_string(),
            ));
        }
        Ok(source)
    })
}

pub fn build_manifest(source: &MangayomiSource, index: usize) -> AppResult<Manifest> {
    let id = format!("mangayomi.{}", sanitize_identity(&source.identity(), index));
    let manifest = Manifest {
        id,
        name: source.name.trim().to_string(),
        version: if source.version.trim().is_empty() {
            "0.0.0".to_string()
        } else {
            source.version.trim().to_string()
        },
        api_version: "0.1".to_string(),
        entry: "index.js".to_string(),
        lang: if source.lang.trim().is_empty() {
            "all".to_string()
        } else {
            source.lang.trim().to_string()
        },
        base_url: source.base_url.trim().to_string(),
        min_host: "0.26.0".to_string(),
        nsfw: source.is_nsfw,
        icon: None,
        permissions: Permissions {
            hosts: vec!["*".to_string()],
            storage_kb: Some(512),
        },
    };
    manifest.validate()?;
    Ok(manifest)
}

pub fn wrap_code(code: &str, index: usize) -> String {
    let adapter = ADAPTER_TEMPLATE.replace("__SOURCE_INDEX__", &index.to_string());
    format!("{BOOTSTRAP}\n;{code}\n;{adapter}\n")
}

fn sanitize_identity(raw: &str, index: usize) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('-');
        }
    }
    if out.is_empty() {
        out = format!("source-{index}");
    }
    out.truncate(96);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext_host::{run, ExtPackage, ExtStorage, HostLimits, MemoryStorage};
    use serde_json::json;
    use std::sync::Arc;

    const STUB: &str = r#"
const mangayomiSources = [
  {
    name: "Stub Source",
    id: 4242,
    baseUrl: "https://example.com",
    lang: "en",
    version: "1.2.3",
    isManga: false,
    itemType: 1,
    sourceCodeUrl: "https://example.com/stub.js"
  }
];

class DefaultExtension extends MProvider {
  constructor() { super(); this.client = new Client(); }
  async search(query, page) {
    await Promise.resolve();
    return {
      list: [{ name: "Show " + query, link: "https://example.com/show/1", imageUrl: "https://example.com/1.jpg" }],
      hasNextPage: true
    };
  }
  async getDetail(url) {
    return {
      name: "Show",
      imageUrl: "https://example.com/1.jpg",
      description: "A stub",
      genre: ["Action"],
      status: 0,
      chapters: [
        { name: "Episode 1", url: "episode-1" },
        { name: "Episode 2", url: "episode-2" }
      ]
    };
  }
  async getVideoList(url) {
    return [{
      url: "https://cdn.example.com/index.m3u8",
      quality: "720p",
      headers: { Referer: "https://example.com/" },
      subtitles: [{ file: "https://cdn.example.com/sub.vtt", label: "English" }]
    }];
  }
}
"#;

    fn storage() -> Arc<dyn ExtStorage> {
        Arc::new(MemoryStorage::default())
    }

    fn limits() -> HostLimits {
        HostLimits {
            timeout: Duration::from_secs(5),
            resolve_timeout: Duration::from_secs(10),
            ..HostLimits::default()
        }
    }

    fn package() -> (ExtPackage, String) {
        let preview = preview(STUB).unwrap();
        assert_eq!(preview.sources.len(), 1);
        let source = source_at(STUB, preview.sources[0].index).unwrap();
        let manifest = build_manifest(&source, preview.sources[0].index).unwrap();
        let code = wrap_code(STUB, preview.sources[0].index);
        assert_eq!(manifest.permissions.hosts, vec!["*".to_string()]);
        assert_eq!(manifest.id, "mangayomi.4242");
        (ExtPackage { manifest, code }, "mangayomi.4242".to_string())
    }

    #[test]
    fn preview_reads_anime_sources() {
        let preview = preview(STUB).unwrap();
        assert!(preview.has_provider);
        assert_eq!(preview.sources.len(), 1);
        assert_eq!(preview.sources[0].name, "Stub Source");
        assert_eq!(preview.sources[0].version, "1.2.3");
    }

    #[test]
    fn preview_rejects_manga_sources() {
        let code = r#"
          const mangayomiSources = [{
            name: "Manga", id: 1, baseUrl: "https://example.com", isManga: true
          }];
          class DefaultExtension extends MProvider {}
        "#;
        let err = preview(code).unwrap_err();
        assert!(err.to_string().contains("no usable anime sources"));
    }

    #[test]
    fn wrapped_provider_maps_all_four_operations() {
        let (package, id) = package();
        let storage = storage();

        let search = run(
            &package,
            "search",
            json!(["Naruto", 1]),
            storage.clone(),
            &limits(),
        )
        .unwrap();
        assert_eq!(id, "mangayomi.4242");
        assert_eq!(search["items"][0]["title"], "Show Naruto");
        assert_eq!(search["items"][0]["id"], "https://example.com/show/1");
        assert_eq!(search["hasNext"], true);

        let show = run(
            &package,
            "getShow",
            json!(["https://example.com/show/1"]),
            storage.clone(),
            &limits(),
        )
        .unwrap();
        assert_eq!(show["title"], "Show");
        assert_eq!(show["status"], "airing");
        assert_eq!(show["genres"][0], "Action");

        let episodes = run(
            &package,
            "getEpisodes",
            json!(["https://example.com/show/1"]),
            storage.clone(),
            &limits(),
        )
        .unwrap();
        assert_eq!(episodes.as_array().unwrap().len(), 2);
        assert_eq!(episodes[0]["number"], 1);
        assert_eq!(episodes[1]["number"], 2);

        let streams = run(
            &package,
            "resolveEpisode",
            json!(["episode-1"]),
            storage,
            &limits(),
        )
        .unwrap();
        assert_eq!(streams[0]["url"], "https://cdn.example.com/index.m3u8");
        assert_eq!(streams[0]["type"], "hls");
        assert_eq!(streams[0]["headers"]["Referer"], "https://example.com/");
        assert_eq!(
            streams[0]["subtitles"][0]["url"],
            "https://cdn.example.com/sub.vtt"
        );
    }
}
