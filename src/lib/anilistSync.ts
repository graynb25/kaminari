// AniList write queue wrappers. The queue lives in Rust and never needs a token here.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AniListPushResult = {
  pushed: number;
  conflicts: number;
  pending: number;
};

export const ANILIST_SYNC_CONFLICT_EVENT = "anilist-sync-conflict";

export function anilistSyncPending(): Promise<number> {
  return invoke<number>("anilist_sync_pending");
}

export function anilistSyncPush(): Promise<AniListPushResult> {
  return invoke<AniListPushResult>("anilist_sync_push");
}

export function onAniListSyncConflict(
  handler: (result: AniListPushResult) => void,
): Promise<UnlistenFn> {
  return listen<AniListPushResult>(ANILIST_SYNC_CONFLICT_EVENT, (event) => {
    handler(event.payload);
  });
}
