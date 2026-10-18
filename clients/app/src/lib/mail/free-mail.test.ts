import { describe, expect, it } from "vitest";

import { isFreeMailDomain } from "./free-mail";

describe("free mail domains", () => {
  it("knows a free mail provider, whatever the case", () => {
    expect(isFreeMailDomain("gmail.com")).toBe(true);
    expect(isFreeMailDomain(" Outlook.com ")).toBe(true);
    expect(isFreeMailDomain("weekly-ledger.example")).toBe(false);
  });

  it("knows a provider's subdomains, as the server's registrable domain does", () => {
    expect(isFreeMailDomain("mail.yahoo.fr")).toBe(true);
    expect(isFreeMailDomain("smtp.qq.com")).toBe(true);
    expect(isFreeMailDomain("notqq.com")).toBe(false);
    expect(isFreeMailDomain("qq.com.example")).toBe(false);
  });
});
