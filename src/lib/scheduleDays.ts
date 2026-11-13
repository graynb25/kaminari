// Pure day grouping for the airing schedule. Times stay Unix seconds.
// Callers pass the display zone. No Svelte imports here.

import type { AiringScheduleEntry } from "./types";

export type ScheduleDay = {
  key: string;
  weekday: string;
  monthDay: string;
  fullLabel: string;
  isToday: boolean;
  entries: AiringScheduleEntry[];
};

function part(parts: Intl.DateTimeFormatPart[], type: string): string {
  return parts.find((item) => item.type === type)?.value ?? "";
}

export function dayKey(airingAt: number, timeZone?: string): string {
  const date = new Date(airingAt * 1000);
  const parts = new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    timeZone,
  }).formatToParts(date);
  return `${part(parts, "year")}-${part(parts, "month")}-${part(parts, "day")}`;
}

function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

function padKey(value: number, width: number): string {
  return String(value).padStart(width, "0");
}

function shiftCalendar(year: number, month: number, day: number, offset: number): string {
  let y = year;
  let m = month;
  let d = day;
  for (let i = 0; i < offset; i += 1) {
    d += 1;
    if (d > daysInMonth(y, m)) {
      d = 1;
      m += 1;
      if (m > 12) {
        m = 1;
        y += 1;
      }
    }
  }
  return `${padKey(y, 4)}-${padKey(m, 2)}-${padKey(d, 2)}`;
}

function instantForKey(key: string, timeZone?: string): Date {
  const [y, m, d] = key.split("-").map(Number);
  let instant = Date.UTC(y, m - 1, d, 12, 0, 0);
  for (let i = 0; i < 5; i += 1) {
    const current = dayKey(Math.floor(instant / 1000), timeZone);
    if (current === key) break;
    if (current < key) instant += 12 * 3600 * 1000;
    else instant -= 12 * 3600 * 1000;
  }
  return new Date(instant);
}

export function rollingDays(now: number, timeZone?: string, count = 7): ScheduleDay[] {
  if (count <= 0) return [];
  const startKey = dayKey(now, timeZone);
  const [sy, sm, sd] = startKey.split("-").map(Number);

  const weekdayFmt = new Intl.DateTimeFormat(undefined, { weekday: "short", timeZone });
  const monthDayFmt = new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    timeZone,
  });
  const fullFmt = new Intl.DateTimeFormat(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
    timeZone,
  });

  const days: ScheduleDay[] = [];
  for (let i = 0; i < count; i += 1) {
    const key = shiftCalendar(sy, sm, sd, i);
    const sample = instantForKey(key, timeZone);
    days.push({
      key,
      weekday: weekdayFmt.format(sample),
      monthDay: monthDayFmt.format(sample),
      fullLabel: fullFmt.format(sample),
      isToday: i === 0,
      entries: [],
    });
  }
  return days;
}

export function groupByDay(
  entries: AiringScheduleEntry[],
  days: ScheduleDay[],
  timeZone?: string,
): ScheduleDay[] {
  const index = new Map(days.map((day, i) => [day.key, i]));
  const grouped: ScheduleDay[] = days.map((day) => ({ ...day, entries: [] }));
  for (const entry of entries) {
    const key = dayKey(entry.airingAt, timeZone);
    const at = index.get(key);
    if (at !== undefined) grouped[at].entries.push(entry);
  }
  for (const day of grouped) {
    day.entries.sort((a, b) => a.airingAt - b.airingAt);
  }
  return grouped;
}

export function nextDayWithEntries(days: ScheduleDay[], selectedKey?: string | null): string | null {
  if (days.length === 0) return null;
  const selectedIndex = selectedKey ? days.findIndex((day) => day.key === selectedKey) : -1;
  if (selectedIndex >= 0) {
    if (days[selectedIndex].entries.length > 0) return days[selectedIndex].key;
    for (let i = selectedIndex + 1; i < days.length; i += 1) {
      if (days[i].entries.length > 0) return days[i].key;
    }
    for (let i = 0; i < selectedIndex; i += 1) {
      if (days[i].entries.length > 0) return days[i].key;
    }
    return null;
  }
  for (const day of days) {
    if (day.entries.length > 0) return day.key;
  }
  return null;
}

export function laterEntries(
  entries: AiringScheduleEntry[],
  days: ScheduleDay[],
  now: number,
  timeZone?: string,
): AiringScheduleEntry[] {
  const keys = new Set(days.map((day) => day.key));
  return entries
    .filter((entry) => entry.airingAt > now && !keys.has(dayKey(entry.airingAt, timeZone)))
    .sort((a, b) => a.airingAt - b.airingAt);
}
