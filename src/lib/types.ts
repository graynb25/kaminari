// Shared shapes for the Svelte frontend. Mirrors the Rust commands contract.
// Keep ids numeric here. Source ids that can exceed 2 to the 53 stay strings
// in the extension layer, never in these AniList and library types.

export type MediaTitle = {
  romaji?: string | null;
  english?: string | null;
  native?: string | null;
};

export type CoverImage = {
  extraLarge?: string | null;
  large?: string | null;
  color?: string | null;
};

export type Media = {
  id: number;
  title: MediaTitle;
  coverImage?: CoverImage | null;
  bannerImage?: string | null;
  description?: string | null;
  genres?: string[] | null;
  averageScore?: number | null;
  seasonYear?: number | null;
  format?: string | null;
  status?: string | null;
  episodes?: number | null;
};

export type Boot = {
  title: string;
  anilistEndpoint: string;
  limits: {
    extTimeoutSecs: number;
    extResolveTimeoutSecs: number;
    extMemoryBytes: number;
  };
};

export type LibraryItem = {
  media: Media;
  status: string;
  addedAt: number;
  watchedCount: number;
};

export type ContinueItem = {
  media: Media;
  episode: number;
  positionSec: number;
  durationSec: number;
  watchedCount: number;
  totalEpisodes: number;
  updatedAt: number;
};

export type LegacySource = {
  name: string;
  lang: string;
  id: string;
  baseUrl: string;
};

export type LegacyExtension = {
  name: string;
  pkg: string;
  apk: string;
  lang: string;
  code: number;
  version: string;
  nsfw: number;
  torrent: number;
  sources: LegacySource[];
};

export type InstalledSource = {
  id: string;
  name: string;
  version: string;
  enabled: boolean;
  sourceCount: number;
  nsfw: boolean;
};

export type JsExtension = {
  id: string;
  name: string;
  version: string;
  apiVersion: string;
  enabled: boolean;
  trust: string;
  installedAt: number;
  hosts: string[];
};

export type JsSearchItem = {
  id: string;
  title: string;
  cover?: string | null;
  year?: number | null;
  score?: number | null;
  episodes?: number | null;
};

export type JsSearchResult = {
  items: JsSearchItem[];
  hasNext?: boolean;
};

export type JsShowDetail = {
  id: string;
  title: string;
  cover?: string | null;
  banner?: string | null;
  synopsis?: string | null;
  year?: number | null;
  genres?: string[] | null;
  status?: string | null;
};

export type JsEpisode = {
  id: string;
  showId?: string | null;
  number: number;
  title?: string | null;
  thumbnail?: string | null;
  airedAt?: string | null;
  durationSec?: number | null;
};

export type JsStream = {
  url: string;
  type: string;
  quality?: string | null;
  headers?: Record<string, string> | null;
  subtitles?: { lang: string; url: string; label?: string }[] | null;
};

export type PlaybackSubtitle = {
  url: string;
  lang: string;
  label?: string | null;
};

export type PlaybackRequest = {
  url: string;
  title?: string;
  headers?: Record<string, string>;
  subtitles?: PlaybackSubtitle[];
  startSec?: number;
};

export type PlaybackInfo = {
  mpvPath: string;
  pid: number;
  title?: string | null;
};

export type PlayerStatus = {
  found: boolean;
  path?: string | null;
  version?: string | null;
};

export type HistoryItem = {
  media: Media;
  episode: number;
  watchedAt: number;
};

export type View = "browse" | "library" | "history" | "sources" | "settings";

export type RailState = {
  items: Media[];
  loading: boolean;
  error: string | null;
};

export type ToastTone = "info" | "success" | "error";

export type Toast = {
  id: number;
  text: string;
  tone: ToastTone;
};
