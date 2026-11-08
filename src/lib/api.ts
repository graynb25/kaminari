// Typed wrappers over the Tauri commands that exist today.
// If a command is missing in Rust, this file is the one place that changes.

import { invoke } from "@tauri-apps/api/core";
import type {
  Boot,
  ContinueItem,
  HistoryItem,
  InstalledSource,
  JsExtension,
  JsSearchResult,
  LegacyExtension,
  LibraryItem,
  Media,
} from "./types";

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

export function setLibraryStatus(id: number, status: string): Promise<void> {
  return invoke<void>("library_set_status", { id, status });
}

export function watchedList(anilistId: number): Promise<number[]> {
  return invoke<number[]>("watched_list", { anilistId });
}

export function watchedSet(media: Media, episode: number, watched: boolean): Promise<number[]> {
  return invoke<number[]>("watched_set", { media, episode, watched });
}

export function watchedMarkAll(
  media: Media,
  through: number,
  watched: boolean,
): Promise<number[]> {
  return invoke<number[]>("watched_mark_all", { media, through, watched });
}

export function continueWatching(): Promise<ContinueItem[]> {
  return invoke<ContinueItem[]>("continue_watching");
}

export function historyList(limit = 100): Promise<HistoryItem[]> {
  return invoke<HistoryItem[]>("history_list", { limit });
}

export function registryFetch(url: string): Promise<LegacyExtension[]> {
  return invoke<LegacyExtension[]>("registry_fetch", { url });
}

export function sourceList(): Promise<InstalledSource[]> {
  return invoke<InstalledSource[]>("source_list");
}

export function sourceInstall(extension: LegacyExtension): Promise<void> {
  return invoke<void>("source_install", { extension });
}

export function sourceRemove(id: string): Promise<void> {
  return invoke<void>("source_remove", { id });
}

export function sourceToggle(id: string, enabled: boolean): Promise<void> {
  return invoke<void>("source_toggle", { id, enabled });
}

export function jsExtensionList(): Promise<JsExtension[]> {
  return invoke<JsExtension[]>("js_extension_list");
}

export function jsExtensionInstallSample(): Promise<void> {
  return invoke<void>("js_extension_install_sample");
}

export function jsExtensionInstallPath(path: string): Promise<void> {
  return invoke<void>("js_extension_install_path", { path });
}

export function jsExtensionRemove(id: string): Promise<void> {
  return invoke<void>("js_extension_remove", { id });
}

export function jsExtensionTest(id: string, query: string): Promise<JsSearchResult> {
  return invoke<JsSearchResult>("js_extension_test", { id, query });
}
