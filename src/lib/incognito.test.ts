import { describe, expect, it } from "vitest";
import { INCOGNITO_KEY, incognitoEnabled, setIncognitoEnabled } from "./incognito";

function memoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    values,
  };
}

describe("incognito", () => {
  it("defaults to off", () => {
    expect(incognitoEnabled(memoryStorage())).toBe(false);
    expect(incognitoEnabled(null)).toBe(false);
  });

  it("reads both stored true forms", () => {
    expect(incognitoEnabled(memoryStorage({ [INCOGNITO_KEY]: "true" }))).toBe(true);
    expect(incognitoEnabled(memoryStorage({ [INCOGNITO_KEY]: "1" }))).toBe(true);
  });

  it("stores off and on", () => {
    const storage = memoryStorage();
    setIncognitoEnabled(true, storage);
    expect(storage.values.get(INCOGNITO_KEY)).toBe("true");
    expect(incognitoEnabled(storage)).toBe(true);
    setIncognitoEnabled(false, storage);
    expect(storage.values.get(INCOGNITO_KEY)).toBe("false");
    expect(incognitoEnabled(storage)).toBe(false);
  });
});
