export const ONBOARDING_KEY = "kaminari.onboardingDone";

type ReadStorage = Pick<Storage, "getItem">;
type WriteStorage = Pick<Storage, "setItem" | "removeItem">;

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function shouldShowOnboarding(storage: ReadStorage | null = browserStorage()): boolean {
  if (!storage) return false;
  try {
    return storage.getItem(ONBOARDING_KEY) !== "true";
  } catch {
    return false;
  }
}

export function dismissOnboarding(storage: WriteStorage | null = browserStorage()): void {
  if (!storage) return;
  try {
    storage.setItem(ONBOARDING_KEY, "true");
  } catch {
    // Storage can be blocked. The card will show again next launch.
  }
}

export function resetOnboarding(storage: WriteStorage | null = browserStorage()): void {
  if (!storage) return;
  try {
    storage.removeItem(ONBOARDING_KEY);
  } catch {
    // Storage can be blocked. Nothing to reset.
  }
}
