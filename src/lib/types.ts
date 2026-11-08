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
};

export type View = "browse" | "library" | "sources" | "settings";

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
