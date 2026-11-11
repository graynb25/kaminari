// Recent search chips. Incognito keeps new entries in memory only: nothing is
// written to localStorage while the flag is on.

export const RECENT_SEARCH_KEY = "kaminari.recentSearches";
export const MAX_RECENT_SEARCHES = 8;

type ReadStorage = Pick<Storage, "getItem">;
type WriteStorage = Pick<Storage, "setItem" | "removeItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function loadRecentSearches(
  storage: ReadStorage | null = browserStorage(),
): string[] {
  if (!storage) return [];
  try {
    const raw = storage.getItem(RECENT_SEARCH_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? parsed
          .filter((value): value is string => typeof value === "string")
          .slice(0, MAX_RECENT_SEARCHES)
      : [];
  } catch {
    return [];
  }
}

export function rememberSearch(
  value: string,
  recent: string[],
  persist: boolean,
  storage: WriteStorage | null = browserStorage(),
): string[] {
  const next = [value, ...recent.filter((entry) => entry !== value)].slice(
    0,
    MAX_RECENT_SEARCHES,
  );
  if (!persist || !storage) return next;
  try {
    storage.setItem(RECENT_SEARCH_KEY, JSON.stringify(next));
  } catch {
    // Private mode or blocked storage. The list stays in memory.
  }
  return next;
}

export function forgetRecentSearches(
  storage: WriteStorage | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.removeItem(RECENT_SEARCH_KEY);
  } catch {
    // Nothing to do.
  }
}
