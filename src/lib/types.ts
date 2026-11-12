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
  nextAiringEpisode?: { airingAt: number; episode: number } | null;
  recommendations?: {
    nodes: { mediaRecommendation?: Media | null }[];
  } | null;
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

export type JsExtension = {
  id: string;
  name: string;
  version: string;
  apiVersion: string;
  kind: string;
  enabled: boolean;
  trust: string;
  installedAt: number;
  hosts: string[];
  nsfw: boolean;
};

export type ExtensionInstallChange = {
  field: string;
  before: string;
  after: string;
};

export type ExtensionInstallPreview = {
  id: string;
  name: string;
  version: string;
  apiVersion: string;
  trust: string;
  newInstall: boolean;
  requiresApproval: boolean;
  codeChanged: boolean;
  nameChanged: boolean;
  versionChanged: boolean;
  manifestChanged: boolean;
  trustChanged: boolean;
  addedHosts: string[];
  removedHosts: string[];
  existingName: string | null;
  existingVersion: string | null;
  existingTrust: string | null;
  changes: ExtensionInstallChange[];
};

export type ExtensionInstallOutcome = {
  installed: boolean;
  approvalToken: string | null;
  preview: ExtensionInstallPreview;
};

export type MangayomiSourcePreview = {
  index: number;
  name: string;
  version: string;
  lang: string;
  baseUrl: string;
  sourceCodeUrl?: string | null;
  nsfw: boolean;
};

export type MangayomiPreview = {
  url: string;
  sha256: string;
  bytes: number;
  sources: MangayomiSourcePreview[];
};

export type SourcePreferenceSwitch = {
  title?: string;
  summary?: string;
  value?: boolean;
};

export type SourcePreferenceList = {
  title?: string;
  summary?: string;
  valueIndex?: number;
  entries?: string[];
  values?: string[];
};

export type SourcePreferenceItem = {
  key: string;
  switchPreferenceCompat?: SourcePreferenceSwitch;
  switchPreference?: SourcePreferenceSwitch;
  listPreference?: SourcePreferenceList;
};

export type SourcePreferences = {
  items: SourcePreferenceItem[];
  values: Record<string, string>;
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
  proxiedUrl?: string | null;
  proxyId?: string | null;
  subtitleProxyIds?: string[] | null;
};

export type DownloadStatus = "running" | "done" | "cancelled" | "error";

export type DownloadRequest = {
  url: string;
  proxiedUrl?: string | null;
  headers?: Record<string, string> | null;
  kind?: string | null;
  folder: string;
  fileName: string;
};

export type DownloadProgress = {
  id: string;
  received: number;
  total: number | null;
  state: DownloadStatus;
  path: string | null;
  message: string | null;
};

export type ProxyStatus = {
  port: number;
  entries: number;
  enabled: boolean;
};

export type ProxyRegistration = {
  proxyUrl: string;
  opaqueId: string;
  expiresInSec: number;
  kind: string;
};

export type ProxyStreamInput = {
  url: string;
  kind: string;
  headers?: Record<string, string> | null;
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

export type MpvEmbedStatus = {
  available: boolean;
  wrapperPresent: boolean;
  libmpvPresent: boolean;
  dllDir?: string | null;
  devDir?: string | null;
  activeLabel?: string | null;
  lastError?: string | null;
};

export type SourceExtensionRef = {
  id: string;
  name: string;
};

export type SourceLibraryItem = {
  extensionId: string;
  extensionName: string;
  showId: string;
  title: string;
  cover?: string | null;
  year?: number | null;
  status?: string | null;
  anilistId?: number | null;
  addedAt: number;
  watchedCount: number;
  lastEpisodeId?: string | null;
  lastEpisode?: number | null;
  lastPosition?: number | null;
  lastDuration?: number | null;
};

export type SourceProgress = {
  episodeId: string;
  episodeNumber: number;
  episodeTitle?: string | null;
  positionSec: number;
  durationSec: number;
  watched: boolean;
  updatedAt: number;
};

export type SourceProgressMark = {
  episodeId: string;
  episodeNumber: number;
  episodeTitle?: string | null;
  durationSec: number;
};

export type SourceContinueItem = {
  extensionId: string;
  extensionName: string;
  showId: string;
  title: string;
  cover?: string | null;
  anilistId?: number | null;
  episodeId: string;
  episodeNumber: number;
  episodeTitle?: string | null;
  positionSec: number;
  durationSec: number;
  watched: boolean;
  updatedAt: number;
};

export type SourceHistoryItem = {
  extensionId: string;
  extensionName: string;
  showId: string;
  title: string;
  cover?: string | null;
  episodeId: string;
  episodeNumber: number;
  episodeTitle?: string | null;
  watched: boolean;
  updatedAt: number;
};

export type HistoryItem = {
  media: Media;
  episode: number;
  watchedAt: number;
};

export type View = "browse" | "schedule" | "library" | "history" | "sources" | "settings";

export type RailState = {
  items: Media[];
  loading: boolean;
  error: string | null;
};

export type ToastTone = "info" | "success" | "error";

export type ToastAction = {
  label: string;
  run: () => void;
};

export type Toast = {
  id: number;
  text: string;
  tone: ToastTone;
  action?: ToastAction;
};

export type AiringScheduleEntry = { anilistId: number; episode: number; airingAt: number };
export type AiringSchedule = { items: AiringScheduleEntry[]; fetchedAt: number | null; stale: boolean; warning: string | null };
