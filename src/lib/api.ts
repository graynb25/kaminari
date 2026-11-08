// Typed wrappers over the Tauri commands that exist today.
// If a command is missing in Rust, this file is the one place that changes.

import { invoke } from "@tauri-apps/api/core";
import type { Boot, LibraryItem, Media } from "./types";

export function bootSnapshot(): Promise<Boot> {
  return invoke<Boot>("boot_snapshot");
}

export function trending(page = 1): Promise<Media[]> {
  return invoke<Media[]>("anilist_trending", { page });
}

export function searchAniList(query: string, page = 1): Promise<Media[]> {
  return invoke<Media[]>("anilist_search", { query, page });
}

export function detail(id: number): Promise<Media> {
  return invoke<Media>("anilist_detail", { id });
}

export type ShelfKind = "trending" | "airing" | "top" | "popular";

export function shelf(kind: ShelfKind, page = 1): Promise<Media[]> {
  return invoke<Media[]>("anilist_shelf", { kind, page });
}

export function listLibrary(): Promise<LibraryItem[]> {
  return invoke<LibraryItem[]>("library_list");
}

export function addToLibrary(media: Media, status = "watching"): Promise<void> {
  return invoke<void>("library_add", { media, status });
}

export function removeFromLibrary(id: number): Promise<void> {
  return invoke<void>("library_remove", { id });
}
