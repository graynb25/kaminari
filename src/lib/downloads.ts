// Download rules shared by the source browser and settings.
// Rust owns the transfer. These helpers only pick names, folders and flags.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isHlsStream, isMp4Stream } from "./streamProxy";
import type { DownloadProgress, DownloadRequest, JsStream } from "./types";

export const DOWNLOADS_FOLDER_KEY = "kaminari.downloadsFolder";
export const DOWNLOADS_CONSENT_KEY = "kaminari.downloadsConsent";
export const DOWNLOAD_PROGRESS_EVENT = "download-progress";

export type DownloadKind = "hls" | "mp4" | "ogv";

type ReadStorage = Pick<Storage, "getItem">;
type WriteStorage = Pick<Storage, "setItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function readDownloadsFolder(
  storage: ReadStorage | null = browserStorage(),
): string | null {
  if (!storage) return null;
  try {
    const value = storage.getItem(DOWNLOADS_FOLDER_KEY)?.trim();
    return value ? value : null;
  } catch {
    return null;
  }
}

export function saveDownloadsFolder(
  folder: string,
  storage: WriteStorage | null = browserStorage(),
): void {
  const value = folder.trim();
  if (!storage || !value) return;
  try {
    storage.setItem(DOWNLOADS_FOLDER_KEY, value);
  } catch {
    // Storage can be blocked. The dialog still returns the folder for this run.
  }
}

export function downloadConsentAccepted(
  storage: ReadStorage | null = browserStorage(),
): boolean {
  if (!storage) return false;
  try {
    const value = storage.getItem(DOWNLOADS_CONSENT_KEY);
    return value === "true" || value === "1";
  } catch {
    return false;
  }
}

export function setDownloadConsent(
  accepted: boolean,
  storage: WriteStorage | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(DOWNLOADS_CONSENT_KEY, accepted ? "true" : "false");
  } catch {
    // A blocked storage keeps the prompt for the next run.
  }
}

export function downloadKind(stream: JsStream): DownloadKind | null {
  if (isHlsStream(stream)) return "hls";
  const type = (stream.type ?? "").trim().toLowerCase();
  const path = stream.url.split(/[?#]/)[0].toLowerCase();
  const proxyPath = (stream.proxiedUrl ?? "").split(/[?#]/)[0].toLowerCase();
  if (type === "m3u8" || proxyPath.endsWith(".m3u8")) return "hls";
  if (isMp4Stream(stream)) return "mp4";
  if (type === "ogv" || type === "ogg" || path.endsWith(".ogv") || path.endsWith(".ogg")) {
    return "ogv";
  }
  return null;
}

function safeName(input: string): string {
  const cleaned = input
    .replace(/[\u0000-\u001f\u007f<>:"/\\|?*]/g, " ")
    .replace(/\.{2,}/g, ".")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/[. ]+$/g, "");
  return cleaned.slice(0, 110).trim() || "Kaminari download";
}

export function downloadFileName(
  showTitle: string,
  episodeNumber: number,
  stream: JsStream,
): string {
  const kind = downloadKind(stream) ?? "mp4";
  const quality = (stream.quality ?? "").trim();
  const pieces = [showTitle, `Episode ${episodeNumber}`];
  if (quality) pieces.push(quality);
  pieces.push(kind === "hls" ? "HLS" : kind.toUpperCase());
  const name = safeName(pieces.join(" - "));
  if (kind === "hls") return name;
  return `${name}.${kind === "ogv" ? "ogv" : "mp4"}`;
}

export function downloadRowKey(episodeId: string, stream: JsStream): string {
  return `${episodeId}|${stream.url}|${stream.proxiedUrl ?? ""}`;
}

export function downloadStart(request: DownloadRequest): Promise<{ id: string }> {
  return invoke<{ id: string }>("download_start", { request });
}

export function downloadCancel(id: string): Promise<void> {
  return invoke<void>("download_cancel", { id });
}

export function onDownloadProgress(
  handler: (progress: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>(DOWNLOAD_PROGRESS_EVENT, (event) => {
    handler(event.payload);
  });
}
