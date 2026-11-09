// AniList OAuth wrappers. Tokens stay in Rust and the OS keychain.
// Payloads here contain only viewer data and user-facing errors.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AniListViewer = {
  id: number;
  name: string;
  avatar?: string | null;
};

export type AniListAuthStatus = {
  connected: boolean;
  viewer: AniListViewer | null;
  error: string | null;
};

export const ANILIST_AUTH_EVENT = "anilist-auth-changed";

export function anilistAuthStart(): Promise<string> {
  return invoke<string>("anilist_auth_start");
}

export function anilistAuthStatus(): Promise<AniListAuthStatus> {
  return invoke<AniListAuthStatus>("anilist_auth_status");
}

export function anilistAuthLogout(): Promise<void> {
  return invoke<void>("anilist_auth_logout");
}

export function onAnilistAuthChanged(
  handler: (status: AniListAuthStatus) => void,
): Promise<UnlistenFn> {
  return listen<AniListAuthStatus>(ANILIST_AUTH_EVENT, (event) => {
    handler(event.payload);
  });
}
