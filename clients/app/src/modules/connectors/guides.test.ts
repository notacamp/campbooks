import { describe, expect, it } from "vitest";

import { guideFor } from "./guides";

describe("guideFor", () => {
  it("picks a guide by the provider database's id first", () => {
    expect(guideFor("ana@bakery.example", "yahoo.com").id).toBe("yahoo");
    expect(guideFor("ana@bakery.example", "me.com").id).toBe("icloud");
    expect(guideFor("ana@bakery.example", "messagingengine.com").id).toBe("fastmail");
    expect(guideFor("ana@bakery.example", "web.de").id).toBe("webde");
    expect(guideFor("ana@bakery.example", "yandex.ru").id).toBe("yandex");
  });

  it("then by the address's domain", () => {
    expect(guideFor("ana@ymail.com").id).toBe("yahoo");
    expect(guideFor("ana@aol.co.uk").id).toBe("aol");
    expect(guideFor("ana@mac.com").id).toBe("icloud");
    expect(guideFor("ana@fastmail.fm").id).toBe("fastmail");
    expect(guideFor("ana@ya.ru").id).toBe("yandex");
  });

  it("gives GMX's German pages unless the address is at gmx.com", () => {
    expect(guideFor("ana@gmx.com", "gmx.com").help).toContain("support.gmx.com");
    expect(guideFor("ana@gmx.de", "gmx.net").imap).toContain("hilfe.gmx.net");
  });

  it("falls back to the general guide, with no link", () => {
    const guide = guideFor("ana@bakery.example", null);
    expect(guide.id).toBe("custom");
    expect(guide.help).toBeUndefined();
  });
});
