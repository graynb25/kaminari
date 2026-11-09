// Dev spike flag for embedded libmpv. Off unless a person opts in. PlayerView
// stays the default path, so a missing flag keeps the shipped behavior.

import { hasStreamHeaders } from "./streamProxy";
import type { JsStream } from "./types";

export const LIBMPV_FLAG_KEY = "kaminari.libmpv";

export type LibmpvStream = Pick<JsStream, "headers" | "proxiedUrl">;

// The wrapper joins header fields into one comma separated string, so headers
// may only reach mpv through the loopback proxy. A stream with headers and no
// proxied URL is not eligible for the embedded backend.
export function libmpvNeedsProxy(stream: LibmpvStream): boolean {
  return hasStreamHeaders(stream) && !stream.proxiedUrl?.trim();
}

type ReadOnlyStorage = Pick<Storage, "getItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function libmpvFlagEnabled(
  storage: ReadOnlyStorage | null = browserStorage(),
): boolean {
  if (!storage) return false;
  try {
    const value = storage.getItem(LIBMPV_FLAG_KEY);
    return value === "true" || value === "1";
  } catch {
    return false;
  }
}

export function setLibmpvFlag(
  enabled: boolean,
  storage: Pick<Storage, "setItem"> | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(LIBMPV_FLAG_KEY, enabled ? "true" : "false");
  } catch {
    // Storage can be blocked. The default stays the WebView player.
  }
}

export function libmpvUrlEnabled(search: string = typeof location === "undefined" ? "" : location.search): boolean {
  try {
    return new URLSearchParams(search).get("libmpv") === "1";
  } catch {
    return false;
  }
}

export function libmpvEnabled(): boolean {
  return libmpvUrlEnabled() || libmpvFlagEnabled();
}
