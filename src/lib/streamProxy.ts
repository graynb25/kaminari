// Pure rules for the loopback proxy. Svelte components stay thin around these.
// P0 proxies MP4 only. HLS with headers still goes direct so mpv can carry them.

import type { JsStream } from "./types";

export const PROXY_FLAG_KEY = "kaminari.proxy";

export function hasStreamHeaders(stream: JsStream): boolean {
  return Object.entries(stream.headers ?? {}).some(
    ([name, value]) => name.trim().length > 0 && value.trim().length > 0,
  );
}

export function isMp4Stream(stream: JsStream): boolean {
  const type = (stream.type ?? "").trim().toLowerCase();
  if (type === "hls") return false;
  const path = stream.url.split(/[?#]/)[0].toLowerCase();
  if (path.endsWith(".m3u8")) return false;
  return type === "mp4" || path.endsWith(".mp4");
}

export function shouldProxy(stream: JsStream): boolean {
  return isMp4Stream(stream) && hasStreamHeaders(stream);
}

export function playbackUrl(stream: JsStream): string {
  const proxied = stream.proxiedUrl?.trim();
  return proxied ? proxied : stream.url;
}

type ReadOnlyStorage = Pick<Storage, "getItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function proxyFlagEnabled(storage: ReadOnlyStorage | null = browserStorage()): boolean {
  if (!storage) return false;
  try {
    const value = storage.getItem(PROXY_FLAG_KEY);
    return value === "true" || value === "1";
  } catch {
    return false;
  }
}

export function setProxyFlag(
  enabled: boolean,
  storage: Pick<Storage, "setItem"> | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(PROXY_FLAG_KEY, enabled ? "true" : "false");
  } catch {
    // Storage can be blocked. The flag holds for this session only.
  }
}
