import { describe, expect, it } from "vitest";

// Where the list is defined: the taught rules engine (`FREE_MAIL`).
import taught from "../../../../../crates/ai/src/decide/taught.rs?raw";

import { FREE_MAIL, isFreeMailDomain } from "./free-mail";

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

  it("is the server's list, entry for entry", () => {
    const block = /pub const FREE_MAIL: &\[&str\] = &\[([\s\S]*?)\];/u.exec(taught)?.[1] ?? "";
    const server = [...block.matchAll(/"([^"]+)"/gu)].map((match) => match[1]);
    expect(server.length).toBeGreaterThan(300);
    expect([...FREE_MAIL].toSorted()).toEqual(server.toSorted());
  });
});
