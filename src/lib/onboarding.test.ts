import { describe, expect, it } from "vitest";
import { dismissOnboarding, ONBOARDING_KEY, resetOnboarding, shouldShowOnboarding } from "./onboarding";

function memoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    removeItem: (key: string) => void values.delete(key),
    values,
  };
}

describe("onboarding", () => {
  it("shows on a fresh profile", () => {
    expect(shouldShowOnboarding(memoryStorage())).toBe(true);
    expect(shouldShowOnboarding(null)).toBe(false);
  });

  it("stays hidden after dismissal", () => {
    const storage = memoryStorage();
    dismissOnboarding(storage);
    expect(storage.values.get(ONBOARDING_KEY)).toBe("true");
    expect(shouldShowOnboarding(storage)).toBe(false);
  });

  it("can be reset", () => {
    const storage = memoryStorage({ [ONBOARDING_KEY]: "true" });
    resetOnboarding(storage);
    expect(shouldShowOnboarding(storage)).toBe(true);
  });
});
