// Pure update helpers. The Tauri plugin owns network, verification and install.
// These keep progress math and release copy testable outside a Tauri window.

import type { DownloadEvent } from "@tauri-apps/plugin-updater";
import { cleanHtml } from "./format";

const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

export type DownloadProgress = {
  received: number;
  total: number | null;
  finished: boolean;
};

export function initialDownloadProgress(): DownloadProgress {
  return { received: 0, total: null, finished: false };
}

export function formatReleaseDate(value?: string): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return `${MONTHS[date.getUTCMonth()]} ${date.getUTCDate()}, ${date.getUTCFullYear()}`;
}

export function releaseNotesExcerpt(body?: string, limit = 280): string {
  const text = cleanHtml(body)
    .replace(/[*#_`>]/g, "")
    .replace(/\s+/g, " ")
    .trim();
  if (text.length <= limit) return text;
  return `${text.slice(0, limit).trimEnd()}...`;
}

export function trackDownload(
  state: DownloadProgress,
  event: DownloadEvent,
): DownloadProgress {
  if (event.event === "Started") {
    const total = event.data.contentLength;
    return {
      received: 0,
      total: total != null && total > 0 ? total : null,
      finished: false,
    };
  }
  if (event.event === "Progress") {
    return { ...state, received: state.received + Math.max(0, event.data.chunkLength) };
  }
  return { ...state, finished: true };
}

export function downloadPercent(state: DownloadProgress): number | null {
  if (state.finished) return 100;
  if (state.total == null || state.total <= 0) return null;
  return Math.min(100, Math.max(0, Math.round((state.received / state.total) * 100)));
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "0 B";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function downloadLabel(state: DownloadProgress): string {
  const received = formatBytes(state.received);
  if (state.finished) return `Downloaded ${received}.`;
  if (state.total == null || state.total <= 0) return `Downloading ${received}.`;
  const percent = downloadPercent(state);
  return `Downloading ${received} of ${formatBytes(state.total)} (${percent}%).`;
}
