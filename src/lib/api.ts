// Typed wrappers over the Tauri commands that exist today.
// If a command is missing in Rust, this file is the one place that changes.

import { invoke } from "@tauri-apps/api/core";
import type {
  Boot,
  ContinueItem,
  HistoryItem,
  InstalledSource,
  JsEpisode,
  JsExtension,
  JsSearchResult,
  JsShowDetail,
  JsStream,
  LegacyExtension,
  LibraryItem,
  MangayomiPreview,
  Media,
  MpvEmbedStatus,
  PlaybackInfo,
  PlaybackRequest,
  PlayerStatus,
  ProxyRegistration,
  ProxyStatus,
  ProxyStreamInput,
  SourceLibraryItem,
  SourcePreferences,
  SourceProgress,
  SourceProgressMark,
  SourceContinueItem,
  SourceHistoryItem,
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

export function historyClear(): Promise<void> {
  return invoke<void>("history_clear");
}

export function historyRemove(anilistId: number, episode: number): Promise<void> {
  return invoke<void>("history_remove", { anilistId, episode });
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

export function mangayomiPreview(url: string): Promise<MangayomiPreview> {
  return invoke<MangayomiPreview>("mangayomi_preview", { url });
}

export function mangayomiInstall(
  url: string,
  sourceIndex: number,
  sha256: string,
): Promise<string> {
  return invoke<string>("mangayomi_install", { url, sourceIndex, sha256 });
}

export function jsExtensionPreferences(id: string): Promise<SourcePreferences> {
  return invoke<SourcePreferences>("js_extension_preferences", { id });
}

export function jsExtensionSetPreference(
  id: string,
  key: string,
  value: string,
): Promise<void> {
  return invoke<void>("js_extension_set_preference", { id, key, value });
}

export function jsExtensionRemove(id: string): Promise<void> {
  return invoke<void>("js_extension_remove", { id });
}

export function jsExtensionTest(id: string, query: string): Promise<JsSearchResult> {
  return invoke<JsSearchResult>("js_extension_test", { id, query });
}

export function jsExtensionDetail(id: string, showId: string): Promise<JsShowDetail> {
  return invoke<JsShowDetail>("js_extension_detail", { id, showId });
}

export function jsExtensionEpisodes(id: string, showId: string): Promise<JsEpisode[]> {
  return invoke<JsEpisode[]>("js_extension_episodes", { id, showId });
}

export function jsExtensionResolve(id: string, episodeId: string): Promise<JsStream[]> {
  return invoke<JsStream[]>("js_extension_resolve", { id, episodeId });
}

export function proxyRegister(
  extensionId: string,
  stream: ProxyStreamInput,
): Promise<ProxyRegistration> {
  return invoke<ProxyRegistration>("proxy_register", { extensionId, stream });
}

export function proxyRevoke(opaqueId: string): Promise<void> {
  return invoke<void>("proxy_revoke", { opaqueId });
}

export function proxyStatus(): Promise<ProxyStatus> {
  return invoke<ProxyStatus>("proxy_status");
}

export function playerPlay(request: PlaybackRequest): Promise<PlaybackInfo> {
  return invoke<PlaybackInfo>("player_play", { request });
}

export function playerStop(): Promise<void> {
  return invoke<void>("player_stop");
}

export function playerStatus(): Promise<PlayerStatus> {
  return invoke<PlayerStatus>("player_status");
}

export function mpvEmbedStatus(): Promise<MpvEmbedStatus> {
  return invoke<MpvEmbedStatus>("mpv_embed_status");
}

export function mpvEmbedInit(windowLabel: string): Promise<string> {
  return invoke<string>("mpv_embed_init", { windowLabel });
}

export function mpvEmbedLoad(
  windowLabel: string,
  url: string,
  headers?: Record<string, string> | null,
): Promise<void> {
  return invoke<void>("mpv_embed_load", {
    windowLabel,
    url,
    headers: headers ?? null,
  });
}

export function mpvEmbedSet(
  windowLabel: string,
  name: string,
  value: string | number | boolean,
): Promise<void> {
  return invoke<void>("mpv_embed_set", { windowLabel, name, value });
}

export function mpvEmbedSeek(
  windowLabel: string,
  seconds: number,
  mode: "absolute" | "relative",
): Promise<void> {
  return invoke<void>("mpv_embed_seek", { windowLabel, seconds, mode });
}

export function mpvEmbedCycleSub(windowLabel: string): Promise<void> {
  return invoke<void>("mpv_embed_cycle_sub", { windowLabel });
}

export function mpvEmbedGet(windowLabel: string, name: string): Promise<unknown> {
  return invoke<unknown>("mpv_embed_get", { windowLabel, name });
}

export function mpvEmbedDestroy(windowLabel: string): Promise<void> {
  return invoke<void>("mpv_embed_destroy", { windowLabel });
}

export function sourceLibraryList(): Promise<SourceLibraryItem[]> {
  return invoke<SourceLibraryItem[]>("source_library_list");
}

export function sourceLibraryAdd(item: {
  extensionId: string;
  showId: string;
  title: string;
  cover?: string | null;
  year?: number | null;
  status?: string | null;
}): Promise<void> {
  return invoke<void>("source_library_add", {
    extensionId: item.extensionId,
    showId: item.showId,
    title: item.title,
    cover: item.cover,
    year: item.year,
    status: item.status,
  });
}

export function sourceLibraryRemove(extensionId: string, showId: string): Promise<void> {
  return invoke<void>("source_library_remove", { extensionId, showId });
}

export function sourceProgressGet(
  extensionId: string,
  showId: string,
): Promise<SourceProgress[]> {
  return invoke<SourceProgress[]>("source_progress_get", { extensionId, showId });
}

export function sourceProgressSet(input: {
  extensionId: string;
  showId: string;
  episodeId: string;
  episodeNumber: number;
  episodeTitle?: string | null;
  positionSec: number;
  durationSec: number;
  watched: boolean;
}): Promise<void> {
  return invoke<void>("source_progress_set", input);
}

export function sourceProgressMarkAll(input: {
  extensionId: string;
  showId: string;
  episodes: SourceProgressMark[];
  watched: boolean;
}): Promise<void> {
  return invoke<void>("source_progress_mark_all", input);
}

export function sourceContinue(limit = 20): Promise<SourceContinueItem[]> {
  return invoke<SourceContinueItem[]>("source_continue", { limit });
}

export function sourceHistory(limit = 100): Promise<SourceHistoryItem[]> {
  return invoke<SourceHistoryItem[]>("source_history", { limit });
}

export function sourceHistoryRemove(
  extensionId: string,
  showId: string,
  episodeId: string,
): Promise<void> {
  return invoke<void>("source_history_remove", { extensionId, showId, episodeId });
}
