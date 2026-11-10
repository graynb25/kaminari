// NSFW sources are hidden by default. The setting is opt-in and local.

export const SHOW_NSFW_KEY = "kaminari.showNsfwSources";

type ReadStorage = Pick<Storage, "getItem">;
type WriteStorage = Pick<Storage, "setItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function showNsfwEnabled(storage: ReadStorage | null = browserStorage()): boolean {
  if (!storage) return false;
  try {
    const value = storage.getItem(SHOW_NSFW_KEY);
    return value === "true" || value === "1";
  } catch {
    return false;
  }
}

export function setShowNsfwEnabled(
  enabled: boolean,
  storage: WriteStorage | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(SHOW_NSFW_KEY, enabled ? "true" : "false");
  } catch {
    // Storage can be blocked. The choice holds for this session only.
  }
}

export function filterNsfwSources<T extends { nsfw: boolean }>(
  items: readonly T[],
  showNsfw: boolean,
): T[] {
  return showNsfw ? [...items] : items.filter((item) => !item.nsfw);
}
