// Pure helpers for the artwork loading lifecycle.
// The Artwork component owns the DOM. These functions own the decisions,
// so the state machine stays testable without rendering images.

export type ArtworkStatus = "empty" | "loading" | "ready" | "failed";

export function artworkStatus(
  src: string | null | undefined,
  loaded: boolean,
  failed: boolean,
): ArtworkStatus {
  if (!src) return "empty";
  if (failed) return "failed";
  if (loaded) return "ready";
  return "loading";
}

export function isArtworkComplete(img: { complete: boolean; naturalWidth: number }): boolean {
  return img.complete && img.naturalWidth > 0;
}

export function isArtworkBroken(img: { complete: boolean; naturalWidth: number }): boolean {
  return img.complete && img.naturalWidth === 0;
}
