import { describe, expect, it } from "vitest";
import { decodeVttDataUrl, isInlineVtt } from "./subtitles";

describe("inline subtitles", () => {
  it("recognizes a VTT data URL", () => {
    expect(isInlineVtt("data:text/vtt;charset=utf-8,WEBVTT")).toBe(true);
    expect(isInlineVtt("https://cdn.example.com/sub.vtt")).toBe(false);
  });

  it("decodes percent encoded WebVTT", () => {
    const text = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello";
    const url = `data:text/vtt;charset=utf-8,${encodeURIComponent(text)}`;
    expect(decodeVttDataUrl(url)).toBe(text);
  });

  it("decodes base64 WebVTT", () => {
    const text = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello";
    const url = `data:text/vtt;base64,${btoa(text)}`;
    expect(decodeVttDataUrl(url)).toBe(text);
  });

  it("returns null for a remote or malformed URL", () => {
    expect(decodeVttDataUrl("https://cdn.example.com/sub.vtt")).toBeNull();
    expect(decodeVttDataUrl("data:text/vtt")).toBeNull();
  });
});
