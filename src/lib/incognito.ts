// Incognito mode. Playback does not write history, progress or AniList
// updates while this is on. The setting stays on until it is turned off.

export const INCOGNITO_KEY = "kaminari.incognito";

type ReadStorage = Pick<Storage, "getItem">;
type WriteStorage = Pick<Storage, "setItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function incognitoEnabled(storage: ReadStorage | null = browserStorage()): boolean {
  if (!storage) return false;
  try {
    const value = storage.getItem(INCOGNITO_KEY);
    return value === "true" || value === "1";
  } catch {
    return false;
  }
}

export function setIncognitoEnabled(
  enabled: boolean,
  storage: WriteStorage | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(INCOGNITO_KEY, enabled ? "true" : "false");
  } catch {
    // Storage can be blocked. The choice holds for this session only.
  }
}
