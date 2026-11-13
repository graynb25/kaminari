// Pure countdown and schedule time helpers. All times are Unix seconds.
// Grouping stays in Rust and scheduleDays, this file only formats.

export type ScheduleTone = "future" | "soon" | "live" | "past";
export type Countdown = { text: string; tone: ScheduleTone; aria: string };

const DAY_SECS = 86400;
const HOUR_SECS = 3600;
const MINUTE_SECS = 60;
const LIVE_WINDOW_SECS = 30 * 60;

function plural(value: number, one: string, many: string): string {
  return `${value} ${value === 1 ? one : many}`;
}

function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

function shortFutureText(diff: number): string {
  if (diff >= DAY_SECS) {
    const days = Math.floor(diff / DAY_SECS);
    const hours = Math.floor((diff % DAY_SECS) / HOUR_SECS);
    return `in ${days}d ${hours}h`;
  }
  if (diff >= HOUR_SECS) {
    const hours = Math.floor(diff / HOUR_SECS);
    const minutes = Math.floor((diff % HOUR_SECS) / MINUTE_SECS);
    return `in ${hours}h ${minutes}m`;
  }
  if (diff >= MINUTE_SECS) {
    const minutes = Math.floor(diff / MINUTE_SECS);
    return `in ${minutes}m`;
  }
  return `in ${Math.floor(diff)}s`;
}

function longRelative(diff: number): string {
  if (diff >= DAY_SECS) {
    const days = Math.floor(diff / DAY_SECS);
    const hours = Math.floor((diff % DAY_SECS) / HOUR_SECS);
    return `in ${plural(days, "day", "days")} ${plural(hours, "hour", "hours")}`;
  }
  if (diff >= HOUR_SECS) {
    const hours = Math.floor(diff / HOUR_SECS);
    const minutes = Math.floor((diff % HOUR_SECS) / MINUTE_SECS);
    return `in ${plural(hours, "hour", "hours")} ${plural(minutes, "minute", "minutes")}`;
  }
  if (diff >= MINUTE_SECS) {
    const minutes = Math.floor(diff / MINUTE_SECS);
    return `in ${plural(minutes, "minute", "minutes")}`;
  }
  return `in ${plural(Math.floor(diff), "second", "seconds")}`;
}

function ariaDatePart(airingAt: number, timeZone?: string): string {
  const date = new Date(airingAt * 1000);
  return new Intl.DateTimeFormat(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
    timeZone,
  }).format(date);
}

export function formatScheduleTime(airingAt: number, timeZone?: string): string {
  const date = new Date(airingAt * 1000);
  return new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
    timeZone,
  }).format(date);
}

export function formatScheduleDate(airingAt: number, timeZone?: string): string {
  const date = new Date(airingAt * 1000);
  return new Intl.DateTimeFormat(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    timeZone,
  }).format(date);
}

export function countdownLabel(airingAt: number, now: number, timeZone?: string): Countdown {
  const diff = airingAt - now;
  const datePart = ariaDatePart(airingAt, timeZone);
  const timePart = formatScheduleTime(airingAt, timeZone);

  if (diff > 0) {
    const text = shortFutureText(diff);
    const tone: ScheduleTone = diff < HOUR_SECS ? "soon" : "future";
    const aria = `${datePart} at ${timePart}, ${longRelative(diff)}`;
    return { text, tone, aria };
  }

  const after = now - airingAt;
  if (after <= LIVE_WINDOW_SECS) {
    return { text: "Airing now", tone: "live", aria: `${datePart} at ${timePart}, Airing now` };
  }
  return { text: "Aired", tone: "past", aria: `${datePart} at ${timePart}, Aired` };
}

export function countdownDetail(airingAt: number, now: number): string {
  const diff = airingAt - now;
  if (diff > 0) {
    if (diff >= DAY_SECS) {
      const days = Math.floor(diff / DAY_SECS);
      const hours = Math.floor((diff % DAY_SECS) / HOUR_SECS);
      const minutes = Math.floor((diff % HOUR_SECS) / MINUTE_SECS);
      return `${days}d ${hours}h ${minutes}m`;
    }
    if (diff >= HOUR_SECS) {
      const hours = Math.floor(diff / HOUR_SECS);
      const minutes = Math.floor((diff % HOUR_SECS) / MINUTE_SECS);
      const seconds = Math.floor(diff % MINUTE_SECS);
      return `${hours}h ${minutes}m ${pad2(seconds)}s`;
    }
    if (diff >= MINUTE_SECS) {
      const minutes = Math.floor(diff / MINUTE_SECS);
      const seconds = Math.floor(diff % MINUTE_SECS);
      return `${minutes}m ${pad2(seconds)}s`;
    }
    return `${Math.floor(diff)}s`;
  }
  if (now - airingAt <= LIVE_WINDOW_SECS) return "Airing now";
  return "Aired";
}
