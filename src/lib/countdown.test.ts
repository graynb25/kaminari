import { describe, expect, it } from "vitest";
import {
  countdownDetail,
  countdownLabel,
  formatScheduleDate,
  formatScheduleTime,
} from "./countdown";

const NOW = 1_769_000_000;

describe("countdownLabel boundaries", () => {
  it("shows days at 24 hours", () => {
    const label = countdownLabel(NOW + 86400, NOW);
    expect(label.text).toBe("in 1d 0h");
    expect(label.tone).toBe("future");
  });

  it("shows hours at 1 hour", () => {
    const label = countdownLabel(NOW + 3600, NOW);
    expect(label.text).toBe("in 1h 0m");
    expect(label.tone).toBe("future");
  });

  it("shows minutes at 1 minute with soon tone", () => {
    const label = countdownLabel(NOW + 60, NOW);
    expect(label.text).toBe("in 1m");
    expect(label.tone).toBe("soon");
  });

  it("shows Airing now at zero", () => {
    const label = countdownLabel(NOW, NOW);
    expect(label.text).toBe("Airing now");
    expect(label.tone).toBe("live");
  });

  it("shows Airing now at 30 minutes after", () => {
    const label = countdownLabel(NOW - 1800, NOW);
    expect(label.text).toBe("Airing now");
    expect(label.tone).toBe("live");
  });

  it("shows Aired past 30 minutes", () => {
    const label = countdownLabel(NOW - 1801, NOW);
    expect(label.text).toBe("Aired");
    expect(label.tone).toBe("past");
  });

  it("uses soon tone under one hour", () => {
    expect(countdownLabel(NOW + 12 * 60, NOW).tone).toBe("soon");
    expect(countdownLabel(NOW + 40, NOW).tone).toBe("soon");
    expect(countdownLabel(NOW + 40, NOW).text).toBe("in 40s");
    expect(countdownLabel(NOW + 5 * 3600, NOW).tone).toBe("future");
  });

  it("carries an exact local aria name", () => {
    const airingAt = NOW + 2 * 86400 + 4 * 3600;
    const label = countdownLabel(airingAt, NOW, "Asia/Tokyo");
    const date = new Date(airingAt * 1000);
    const expectedDate = new Intl.DateTimeFormat(undefined, {
      weekday: "long",
      month: "long",
      day: "numeric",
      timeZone: "Asia/Tokyo",
    }).format(date);
    const expectedTime = formatScheduleTime(airingAt, "Asia/Tokyo");
    expect(label.aria).toContain(expectedDate);
    expect(label.aria).toContain(expectedTime);
    expect(label.aria).toContain("in ");
  });

  it("names live and past rows with date and time", () => {
    const live = countdownLabel(NOW, NOW, "America/New_York");
    expect(live.aria).toContain(formatScheduleTime(NOW, "America/New_York"));
    expect(live.aria).toContain("Airing now");
    const past = countdownLabel(NOW - 5000, NOW, "America/New_York");
    expect(past.aria).toContain("Aired");
  });
});

describe("countdownDetail", () => {
  it("spells out the next-up text", () => {
    expect(countdownDetail(NOW + 141720, NOW)).toBe("1d 15h 22m");
    expect(countdownDetail(NOW + 18728, NOW)).toBe("5h 12m 08s");
    expect(countdownDetail(NOW + 750, NOW)).toBe("12m 30s");
    expect(countdownDetail(NOW + 40, NOW)).toBe("40s");
    expect(countdownDetail(NOW, NOW)).toBe("Airing now");
    expect(countdownDetail(NOW - 1800, NOW)).toBe("Airing now");
    expect(countdownDetail(NOW - 1801, NOW)).toBe("Aired");
  });
});

describe("schedule time formatting", () => {
  const sample = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);

  it("renders the same instant differently in Tokyo and New York", () => {
    const tokyo = formatScheduleTime(sample, "Asia/Tokyo");
    const york = formatScheduleTime(sample, "America/New_York");
    expect(tokyo).not.toBe(york);
    const expectedTokyo = new Intl.DateTimeFormat(undefined, {
      hour: "numeric",
      minute: "2-digit",
      timeZone: "Asia/Tokyo",
    }).format(new Date(sample * 1000));
    const expectedYork = new Intl.DateTimeFormat(undefined, {
      hour: "numeric",
      minute: "2-digit",
      timeZone: "America/New_York",
    }).format(new Date(sample * 1000));
    expect(tokyo).toBe(expectedTokyo);
    expect(york).toBe(expectedYork);
  });

  it("formats dates per zone without a hardcoded zone", () => {
    const tokyo = formatScheduleDate(sample, "Asia/Tokyo");
    const york = formatScheduleDate(sample, "America/New_York");
    expect(tokyo).toContain("14");
    expect(york).toContain("14");
    const expected = new Intl.DateTimeFormat(undefined, {
      weekday: "short",
      month: "short",
      day: "numeric",
      timeZone: "Asia/Tokyo",
    }).format(new Date(sample * 1000));
    expect(tokyo).toBe(expected);
  });
});
