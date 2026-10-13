import { afterEach, describe, expect, it, vi } from "vitest";

import { readTabValue, writeTabValue } from "./tab";

describe("tab values", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    globalThis.sessionStorage.clear();
  });

  it("round-trips a value in this tab's storage under a namespaced key", () => {
    writeTabValue("signIn.returnTo", "/w/1");

    expect(readTabValue("signIn.returnTo")).toBe("/w/1");
    expect(globalThis.sessionStorage.getItem("campbooks.tab.signIn.returnTo")).toBe("/w/1");
  });

  it("returns null for a missing value", () => {
    expect(readTabValue("signIn.returnTo")).toBeNull();
  });

  it("degrades to no-ops when storage throws", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("SecurityError");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("QuotaExceededError");
    });

    expect(() => {
      writeTabValue("signIn.returnTo", "/");
    }).not.toThrow();
    expect(readTabValue("signIn.returnTo")).toBeNull();
  });
});
