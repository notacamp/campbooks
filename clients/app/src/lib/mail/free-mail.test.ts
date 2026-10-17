import { describe, expect, it } from "vitest";

import { isFreeMailDomain } from "./free-mail";

describe("free mail domains", () => {
  it("knows a free mail provider, whatever the case", () => {
    expect(isFreeMailDomain("gmail.com")).toBe(true);
    expect(isFreeMailDomain(" Outlook.com ")).toBe(true);
    expect(isFreeMailDomain("weekly-ledger.example")).toBe(false);
  });
});
