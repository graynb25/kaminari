export type ThemeMode = "system" | "dark" | "light";

export const THEME_MODES: { id: ThemeMode; label: string; icon: string; hint: string }[] = [
  { id: "system", label: "System", icon: "monitor", hint: "Follow Windows" },
  { id: "dark", label: "Dark", icon: "moon", hint: "Storm" },
  { id: "light", label: "Light", icon: "sun", hint: "Daylight" },
];

const MODE_KEY = "kaminari.theme";
const LIGHT_QUERY = "(prefers-color-scheme: light)";

let mode: ThemeMode = "system";
let listening = false;

function systemMode(): "dark" | "light" {
  if (typeof matchMedia === "undefined") return "dark";
  return matchMedia(LIGHT_QUERY).matches ? "light" : "dark";
}

function readMode(): ThemeMode {
  try {
    const raw = localStorage.getItem(MODE_KEY);
    if (raw === "system" || raw === "dark" || raw === "light") return raw;
  } catch {
    // Storage can be blocked. System remains the safe default.
  }
  return "system";
}

function resolved(): "dark" | "light" {
  return mode === "system" ? systemMode() : mode;
}

function paint() {
  const theme = resolved();
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

export function initTheme() {
  mode = readMode();
  paint();
  if (!listening && typeof matchMedia !== "undefined") {
    listening = true;
    matchMedia(LIGHT_QUERY).addEventListener("change", () => {
      if (mode === "system") paint();
    });
  }
}

export function getThemeMode(): ThemeMode {
  return mode;
}

export function setThemeMode(next: ThemeMode) {
  mode = next;
  try {
    localStorage.setItem(MODE_KEY, next);
  } catch {
    // The theme still applies for this session.
  }
  paint();
}
