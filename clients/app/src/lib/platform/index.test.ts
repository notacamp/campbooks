import { describe, expect, it, vi } from "vitest";

import { isTauri } from "./index";

describe("isTauri", () => {
  it("is false in a plain browser", () => {
    expect(isTauri()).toBe(false);
  });

  it("is true when the Tauri runtime is injected", () => {
    vi.stubGlobal("isTauri", true);

    expect(isTauri()).toBe(true);
  });
});
