// Autoplay next episode. Default on, with an explicit off switch in Settings.

export const AUTOPLAY_KEY = "kaminari.autoplayNext";

type ReadStorage = Pick<Storage, "getItem">;
type WriteStorage = Pick<Storage, "setItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function autoplayEnabled(storage: ReadStorage | null = browserStorage()): boolean {
  if (!storage) return true;
  try {
    const value = storage.getItem(AUTOPLAY_KEY);
    if (value === null) return true;
    return value === "true" || value === "1";
  } catch {
    return true;
  }
}

export function setAutoplayEnabled(
  enabled: boolean,
  storage: WriteStorage | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(AUTOPLAY_KEY, enabled ? "true" : "false");
  } catch {
    // Storage can be blocked. The choice holds for this session only.
  }
}
