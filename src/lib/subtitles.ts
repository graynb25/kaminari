// Inline subtitle helpers. Mangayomi sources often return WebVTT or SRT text
// in the subtitle object instead of a URL. Chromium refuses to load a
// data:text/vtt track, so the player converts it to a Blob URL before mounting
// the track element.

export function isInlineVtt(url: string): boolean {
  return url.trim().toLowerCase().startsWith("data:text/vtt");
}

export function decodeVttDataUrl(url: string): string | null {
  if (!isInlineVtt(url)) return null;
  const comma = url.indexOf(",");
  if (comma < 0) return null;
  const meta = url.slice(0, comma).toLowerCase();
  const payload = url.slice(comma + 1);
  try {
    if (meta.includes(";base64")) {
      return atob(payload);
    }
    return decodeURIComponent(payload);
  } catch {
    return null;
  }
}

export function vttBlobUrl(url: string): string | null {
  const text = decodeVttDataUrl(url);
  if (!text) return null;
  try {
    return URL.createObjectURL(new Blob([text], { type: "text/vtt;charset=utf-8" }));
  } catch {
    return null;
  }
}
