// History retention setting. The stored value is a small string so an older
// settings export without the field still imports cleanly and means Forever.

export const HISTORY_RETENTION_KEY = "kaminari.historyRetentionDays";

export type HistoryRetention = "30" | "90" | "365" | "forever";

export const HISTORY_RETENTION_OPTIONS: { id: HistoryRetention; label: string }[] = [
  { id: "30", label: "30 days" },
  { id: "90", label: "90 days" },
  { id: "365", label: "365 days" },
  { id: "forever", label: "Forever" },
];

const VALUES = new Set<string>(HISTORY_RETENTION_OPTIONS.map((option) => option.id));

type ReadStorage = Pick<Storage, "getItem">;
type WriteStorage = Pick<Storage, "setItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function parseHistoryRetention(value: unknown): HistoryRetention {
  return typeof value === "string" && VALUES.has(value)
    ? (value as HistoryRetention)
    : "forever";
}

export function historyRetentionDays(value: HistoryRetention): number | null {
  return value === "forever" ? null : Number(value);
}

export function readHistoryRetention(
  storage: ReadStorage | null = browserStorage(),
): HistoryRetention {
  if (!storage) return "forever";
  try {
    return parseHistoryRetention(storage.getItem(HISTORY_RETENTION_KEY));
  } catch {
    return "forever";
  }
}

export function saveHistoryRetention(
  value: HistoryRetention,
  storage: WriteStorage | null = browserStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(HISTORY_RETENTION_KEY, value);
  } catch {
    // Storage can be blocked. The choice holds for this session only.
  }
}
