import { describe, expect, it } from "vitest";

import { Latest } from "./latest";

describe("a value read later", () => {
  it("holds the value it started with, and the latest one set", () => {
    const box = new Latest("first");
    expect(box.current).toBe("first");
    box.set("second");
    expect(box.current).toBe("second");
  });
});
