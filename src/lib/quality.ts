// Preferred playback quality. The source decides which streams exist; this
// only picks among them. Auto keeps the source order, which is often the
// source author's recommended default.

import type { JsStream } from "./types";

export const QUALITY_KEY = "kaminari.preferredQuality";

export type QualityPreference = "auto" | "best" | "2160" | "1080" | "720" | "480" | "360";

export const QUALITY_OPTIONS: { value: QualityPreference; label: string }[] = [
  { value: "auto", label: "Auto (source order)" },
  { value: "best", label: "Best available" },
  { value: "2160", label: "4K" },
  { value: "1080", label: "1080p" },
  { value: "720", label: "720p" },
  { value: "480", label: "480p" },
  { value: "360", label: "360p" },
];

const VALUES = new Set<string>(QUALITY_OPTIONS.map((option) => option.value));

type ReadStorage = Pick<Storage, "getItem">;
type WriteStorage = Pick<Storage, "setItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function parseQualityPreference(value: unknown): QualityPreference {
  return typeof value === "string" && VALUES.has(value) ? (value as QualityPreference) : "auto";
}

export function readQualityPreference(storage: ReadStorage | null = browserStorage()): QualityPreference {
  if (!storage) return "auto";
  try {
    return parseQualityPreference(storage.getItem(QUALITY_KEY));
  } catch {
    return "auto";
  }
}

export function saveQualityPreference(
  value: QualityPreference,
  storage: WriteStorage | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(QUALITY_KEY, value);
  } catch {
    // Storage can be blocked. The choice holds for this session only.
  }
}

export function streamHeight(stream: Pick<JsStream, "quality">): number | null {
  const text = String(stream.quality ?? "");
  if (!text) return null;
  if (/\b4k\b/i.test(text)) return 2160;
  const resolution = text.match(/(\d{3,4})\s*[x×]\s*(\d{3,4})/i);
  if (resolution) {
    const height = Number(resolution[2]);
    return height >= 144 && height <= 4320 ? height : null;
  }
  const labelled = text.match(/(\d{3,4})\s*p\b/i);
  if (labelled) {
    const height = Number(labelled[1]);
    return height >= 144 && height <= 4320 ? height : null;
  }
  return null;
}

function variantKey(quality: string | null | undefined): string {
  return String(quality ?? "")
    .replace(/^\s*\d{3,4}\s*[x×]\s*\d{3,4}\s*[-\u2013\u2014:|]?\s*/i, "")
    .replace(/^\s*\d{3,4}\s*p\b\s*[-\u2013\u2014:|]?\s*/i, "")
    .trim()
    .toLowerCase();
}

export function pickStream<T extends Pick<JsStream, "quality">>(
  streams: T[],
  preference: QualityPreference,
): T | null {
  if (streams.length === 0) return null;
  if (preference === "auto") return streams[0];

  const measured = streams
    .map((stream, index) => ({ stream, index, height: streamHeight(stream) }))
    .filter((row): row is { stream: T; index: number; height: number } => row.height !== null);
  if (measured.length === 0) return streams[0];

  // Keep the first stream's server/audio variant. A quality change should not
  // silently switch from sub to dub or to a slower server.
  const firstVariant = variantKey(streams[0]?.quality);
  const sameVariant = firstVariant
    ? measured.filter((row) => variantKey(row.stream.quality) === firstVariant)
    : [];
  const pool = sameVariant.length > 0 ? sameVariant : measured;

  if (preference === "best") {
    pool.sort((a, b) => b.height - a.height || a.index - b.index);
    return pool[0].stream;
  }

  const target = Number(preference);
  const exact = pool.find((row) => row.height === target);
  if (exact) return exact.stream;

  const below = pool
    .filter((row) => row.height < target)
    .sort((a, b) => b.height - a.height || a.index - b.index);
  if (below.length > 0) return below[0].stream;

  const above = pool
    .filter((row) => row.height > target)
    .sort((a, b) => a.height - b.height || a.index - b.index);
  return above[0]?.stream ?? streams[0];
}
