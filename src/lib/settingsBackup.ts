// Settings export and import. Plain JSON, small enough to inspect by hand.

import { getAccent, parseAccent, setAccent, type AccentId } from "./accent";
import {
  parseHistoryRetention,
  readHistoryRetention,
  saveHistoryRetention,
  type HistoryRetention,
} from "./historyRetention";
import {
  parseQualityPreference,
  readQualityPreference,
  saveQualityPreference,
  type QualityPreference,
} from "./quality";
import { proxyFlagEnabled, setProxyFlag } from "./streamProxy";
import { getThemeMode, setThemeMode, type ThemeMode } from "./theme";

export type SettingsData = {
  format: "kaminari-settings";
  version: 1;
  appVersion: string;
  exportedAt: string;
  theme: ThemeMode;
  accent: AccentId;
  proxy: boolean;
  quality: QualityPreference;
  historyRetention: HistoryRetention;
};

const FORMAT = "kaminari-settings";
const VERSION = 1;

export function collectSettings(): SettingsData {
  return {
    format: FORMAT,
    version: VERSION,
    appVersion: __APP_VERSION__,
    exportedAt: new Date().toISOString(),
    theme: getThemeMode(),
    accent: getAccent(),
    proxy: proxyFlagEnabled(),
    quality: readQualityPreference(),
    historyRetention: readHistoryRetention(),
  };
}

export function parseSettings(raw: string): SettingsData {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    throw new Error("Settings file is not valid JSON.");
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("Settings file is not a Kaminari settings object.");
  }

  const record = value as Record<string, unknown>;
  if (record.format !== FORMAT) {
    throw new Error("This is not a Kaminari settings file.");
  }
  if (record.version !== VERSION) {
    throw new Error("Unsupported settings version.");
  }

  const theme = record.theme;
  if (theme !== "system" && theme !== "dark" && theme !== "light") {
    throw new Error("Settings file has an invalid theme.");
  }
  if (typeof record.proxy !== "boolean") {
    throw new Error("Settings file has an invalid proxy flag.");
  }

  return {
    format: FORMAT,
    version: VERSION,
    appVersion: typeof record.appVersion === "string" ? record.appVersion : "unknown",
    exportedAt: typeof record.exportedAt === "string" ? record.exportedAt : "",
    theme,
    accent: parseAccent(typeof record.accent === "string" ? record.accent : null),
    proxy: record.proxy,
    quality: parseQualityPreference(record.quality),
    historyRetention: parseHistoryRetention(record.historyRetention),
  };
}

export function applySettings(data: SettingsData): void {
  setThemeMode(data.theme);
  setAccent(data.accent);
  setProxyFlag(data.proxy);
  saveQualityPreference(data.quality);
  saveHistoryRetention(data.historyRetention);
}
