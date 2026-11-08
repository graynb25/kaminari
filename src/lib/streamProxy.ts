// Pure rules for the loopback proxy. Svelte components stay thin around these.
// MP4 and HLS with headers go through the loopback proxy. Headerless media
// stays direct so mpv and public sample URLs keep their current path.

import type { JsStream } from "./types";

export const PROXY_FLAG_KEY = "kaminari.proxy";

export function hasStreamHeaders(stream: JsStream): boolean {
  return Object.entries(stream.headers ?? {}).some(
    ([name, value]) => name.trim().length > 0 && value.trim().length > 0,
  );
}

export function isHlsStream(stream: JsStream): boolean {
  const type = (stream.type ?? "").trim().toLowerCase();
  if (type === "hls") return true;
  const path = stream.url.split(/[?#]/)[0].toLowerCase();
  return path.endsWith(".m3u8");
}

export function isMp4Stream(stream: JsStream): boolean {
  const type = (stream.type ?? "").trim().toLowerCase();
  if (isHlsStream(stream)) return false;
  const path = stream.url.split(/[?#]/)[0].toLowerCase();
  return type === "mp4" || path.endsWith(".mp4");
}

export function shouldProxy(stream: JsStream): boolean {
  return hasStreamHeaders(stream) && (isMp4Stream(stream) || isHlsStream(stream));
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
