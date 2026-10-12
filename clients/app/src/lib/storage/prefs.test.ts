import { afterEach, describe, expect, it, vi } from "vitest";

import { readPref, writePref } from "./prefs";

describe("prefs", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    globalThis.localStorage.clear();
  });

  it("round-trips a preference under a namespaced key", () => {
    writePref("locale", "pt");

    expect(readPref("locale")).toBe("pt");
    expect(globalThis.localStorage.getItem("campbooks.pref.locale")).toBe("pt");
  });

  it("returns null for a missing preference", () => {
    expect(readPref("locale")).toBeNull();
  });

  it("degrades to no-ops when storage throws", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("SecurityError");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("QuotaExceededError");
    });

    expect(() => {
      writePref("locale", "fr");
    }).not.toThrow();
    expect(readPref("locale")).toBeNull();
  });

  it("degrades to no-ops when storage is inaccessible", () => {
    vi.spyOn(globalThis, "localStorage", "get").mockImplementation(() => {
      throw new Error("SecurityError");
    });

    expect(readPref("locale")).toBeNull();
  });
});
