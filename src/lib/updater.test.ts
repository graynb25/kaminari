import { describe, expect, it } from "vitest";
import type { DownloadEvent } from "@tauri-apps/plugin-updater";
import {
  downloadLabel,
  downloadPercent,
  formatReleaseDate,
  initialDownloadProgress,
  missingReleaseFeed,
  releaseNotesExcerpt,
  trackDownload,
  updateInstallMessage,
} from "./updater";

function started(contentLength?: number): DownloadEvent {
  return { event: "Started", data: { contentLength } };
}

function progress(chunkLength: number): DownloadEvent {
  return { event: "Progress", data: { chunkLength } };
}

function finished(): DownloadEvent {
  return { event: "Finished" };
}

describe("updateInstallMessage", () => {
  it("tells Windows users the installer will restart the app", () => {
    const text = updateInstallMessage("0.26.3", true);
    expect(text).toContain("installing");
    expect(text).toContain("close and reopen");
  });

  it("asks other platforms to restart", () => {
    expect(updateInstallMessage("0.26.3", false)).toContain("Restart to apply");
  });
});

describe("missingReleaseFeed", () => {
  it("recognizes the absent release manifest", () => {
    expect(
      missingReleaseFeed(new Error("Could not fetch a valid release JSON from the remote")),
    ).toBe(true);
    expect(missingReleaseFeed("HTTP 404 not found")).toBe(true);
    expect(missingReleaseFeed(new Error("network is unreachable"))).toBe(false);
  });
});

describe("formatReleaseDate", () => {
  it("formats an RFC 3339 date in UTC", () => {
    expect(formatReleaseDate("2026-11-10T09:30:00Z")).toBe("Nov 10, 2026");
  });

  it("leaves an unparseable date alone and skips an empty one", () => {
    expect(formatReleaseDate("soon")).toBe("soon");
    expect(formatReleaseDate()).toBe("");
  });
});

describe("releaseNotesExcerpt", () => {
  it("strips markup and collapses whitespace", () => {
    expect(releaseNotesExcerpt("<p>Fixes <b>seeking</b></p>\n\n* Faster start")).toBe(
      "Fixes seeking Faster start",
    );
  });

  it("truncates long notes with an ASCII ellipsis", () => {
    expect(releaseNotesExcerpt("a".repeat(300), 10)).toBe("aaaaaaaaaa...");
  });
});

describe("download tracking", () => {
  it("resets the count on Started and reads the total", () => {
    const state = trackDownload(initialDownloadProgress(), started(2048));
    expect(state).toEqual({ received: 0, total: 2048, finished: false });
  });

  it("accumulates progress and reports a percentage", () => {
    let state = trackDownload(initialDownloadProgress(), started(2000));
    state = trackDownload(state, progress(500));
    state = trackDownload(state, progress(500));
    expect(state.received).toBe(1000);
    expect(downloadPercent(state)).toBe(50);
    expect(downloadLabel(state)).toBe("Downloading 1000 B of 2.0 KB (50%).");
  });

  it("treats an unknown total as indeterminate", () => {
    let state = trackDownload(initialDownloadProgress(), started());
    state = trackDownload(state, progress(512));
    expect(state.total).toBeNull();
    expect(downloadPercent(state)).toBeNull();
    expect(downloadLabel(state)).toBe("Downloading 512 B.");
  });

  it("finishes at one hundred percent", () => {
    let state = trackDownload(initialDownloadProgress(), started(4096));
    state = trackDownload(state, progress(4096));
    state = trackDownload(state, finished());
    expect(state.finished).toBe(true);
    expect(downloadPercent(state)).toBe(100);
    expect(downloadLabel(state)).toBe("Downloaded 4.0 KB.");
  });
});
