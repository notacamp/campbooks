import { describe, expect, it } from "vitest";

import { isAllowedLink } from "./extensions";

describe("isAllowedLink", () => {
  it("allows the web and mail and nothing else", () => {
    expect(isAllowedLink("https://example.com/a")).toBe(true);
    expect(isAllowedLink("HTTP://example.com")).toBe(true);
    expect(isAllowedLink("mailto:ana@example.com")).toBe(true);
    expect(isAllowedLink("javascript:alert(1)")).toBe(false);
    expect(isAllowedLink("data:text/html,x")).toBe(false);
    expect(isAllowedLink("/relative")).toBe(false);
    expect(isAllowedLink("")).toBe(false);
  });
});
