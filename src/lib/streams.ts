// Helpers for Mangayomi HLS variant handling. Some sources return one
// video-only media playlist per quality and keep the audio-bearing master
// playlist in `originalUrl`. Playing the variant alone is silent, so the
// player should use the master and switch levels inside hls.js.
import type { JsStream } from "./types";

function isHttpUrl(value: string | null | undefined): boolean {
  if (!value) return false;
  const trimmed = value.trim();
  return /^https?:\/\//i.test(trimmed);
}

function isM3u8Url(value: string | null | undefined): boolean {
  if (!value) return false;
  const path = value.split(/[?#]/)[0]?.toLowerCase() ?? "";
  return path.endsWith(".m3u8");
}

function isHlsStream(stream: Pick<JsStream, "type" | "url">): boolean {
  const type = (stream.type ?? "").trim().toLowerCase();
  if (type === "hls") return true;
  const path = stream.url.split(/[?#]/)[0]?.toLowerCase() ?? "";
  return path.endsWith(".m3u8");
}

export function isMasterCandidate(stream: JsStream): boolean {
  if (!isHlsStream(stream)) return false;
  const original = stream.originalUrl?.trim() ?? "";
  if (!original) return false;
  if (original === stream.url) return false;
  if (!isHttpUrl(original)) return false;
  if (!isM3u8Url(original)) return false;
  return true;
}

function audioHint(quality: string | null | undefined): string | null {
  const text = String(quality ?? "");
  if (/\[dub\]/i.test(text)) return "en";
  if (/\[sub\]/i.test(text)) return "ja";
  return null;
}

export function audioMasterFor(
  stream: JsStream,
  list: JsStream[],
): JsStream | null {
  if (!isMasterCandidate(stream)) return null;
  const master = stream.originalUrl?.trim() ?? "";
  const group = list.filter(
    (row) => isMasterCandidate(row) && row.originalUrl?.trim() === master,
  );
  if (group.length < 2) return null;
  const variants = new Set(group.map((row) => row.url));
  if (variants.size < 2) return null;
  return {
    url: master,
    originalUrl: null,
    audioLanguage: audioHint(stream.quality),
    type: "hls",
    quality: "Auto",
    headers: stream.headers ?? null,
    subtitles: stream.subtitles ?? null,
  };
}

export function uniqueStreams<T extends Pick<JsStream, "url">>(list: T[]): T[] {
  const seen = new Set<string>();
  const out: T[] = [];
  for (const row of list) {
    if (seen.has(row.url)) continue;
    seen.add(row.url);
    out.push(row);
  }
  return out;
}

export function collapseHlsVariants(list: JsStream[]): JsStream[] {
  const deduped = uniqueStreams(list);
  const emitted = new Set<string>();
  const seenUrl = new Set<string>();
  const out: JsStream[] = [];
  for (const row of deduped) {
    if (isMasterCandidate(row)) {
      const key = row.originalUrl?.trim() ?? "";
      const group = deduped.filter(
        (other) =>
          isMasterCandidate(other) && other.originalUrl?.trim() === key,
      );
      const qualifies =
        group.length >= 2 &&
        new Set(group.map((item) => item.url)).size >= 2;
      if (qualifies) {
        if (emitted.has(key)) continue;
        const master = audioMasterFor(row, deduped);
        if (master) {
          if (seenUrl.has(master.url)) {
            emitted.add(key);
            continue;
          }
          emitted.add(key);
          seenUrl.add(master.url);
          out.push(master);
          continue;
        }
      }
    }
    if (seenUrl.has(row.url)) continue;
    seenUrl.add(row.url);
    out.push(row);
  }
  return out;
}
