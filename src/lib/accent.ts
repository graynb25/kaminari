export type AccentId = "violet" | "ocean" | "forest" | "ember" | "rose";

export const ACCENTS: {
  id: AccentId;
  label: string;
  swatch: string;
}[] = [
  { id: "violet", label: "Violet", swatch: "#8b8aff" },
  { id: "ocean", label: "Ocean", swatch: "#6ea8ff" },
  { id: "forest", label: "Forest", swatch: "#62c99a" },
  { id: "ember", label: "Ember", swatch: "#ff9457" },
  { id: "rose", label: "Rose", swatch: "#ff7fa4" },
];

const ACCENT_KEY = "kaminari.accent";

let accent: AccentId = "violet";

export function parseAccent(raw: string | null): AccentId {
  return ACCENTS.some((entry) => entry.id === raw)
    ? (raw as AccentId)
    : "violet";
}

function readAccent(): AccentId {
  try {
    return parseAccent(localStorage.getItem(ACCENT_KEY));
  } catch {
    // Storage can be blocked. Violet remains the signature default.
  }
  return "violet";
}

function paint() {
  if (accent === "violet") {
    delete document.documentElement.dataset.accent;
  } else {
    document.documentElement.dataset.accent = accent;
  }
}

export function initAccent() {
  accent = readAccent();
  paint();
}

export function getAccent(): AccentId {
  return accent;
}

export function setAccent(next: AccentId) {
  accent = next;
  try {
    localStorage.setItem(ACCENT_KEY, next);
  } catch {
    // The accent still applies for this session.
  }
  paint();
}
