import { describe, expect, it } from "vitest";

import { carryRecipients, filledFor, type Recipients } from "./switch-mode";

const SAM = { name: "Sam Rivers", address: "sam@example.com" };
const MAYA = { name: "Maya Okafor", address: "maya@example.com" };
const RUI = { name: "Rui", address: "rui@example.com" };
const LEA = { name: "Lea", address: "lea@example.com" };
const BEA = { name: "Bea", address: "bea@example.com" };

// Maya wrote to Sam and Rui, copying Lea.
const RECEIVED = {
  direction: "inbound" as const,
  from: MAYA,
  to: [SAM, RUI],
  cc: [LEA],
  reply_to: [],
};

const none: Recipients = { to: [], cc: [], bcc: [] };

describe("filledFor", () => {
  it("fills what the server fills, never the inbox's own address", () => {
    expect(filledFor("reply", RECEIVED, "SAM@example.com")).toEqual({ ...none, to: [MAYA] });
    expect(filledFor("reply_all", RECEIVED, "sam@example.com")).toEqual({
      to: [MAYA, RUI],
      cc: [LEA],
      bcc: [],
    });
    expect(filledFor("forward", RECEIVED, "sam@example.com")).toEqual(none);
  });

  it("answers Reply-To, and the same people again for mail you sent", () => {
    expect(filledFor("reply", { ...RECEIVED, reply_to: [BEA] }, "sam@example.com").to).toEqual([
      BEA,
    ]);
    const sent = { ...RECEIVED, direction: "outbound" as const, from: SAM, to: [MAYA] };
    expect(filledFor("reply_all", sent, "sam@example.com")).toEqual({
      to: [MAYA],
      cc: [LEA],
      bcc: [],
    });
  });
});

describe("carryRecipients", () => {
  const reply = filledFor("reply", RECEIVED, "sam@example.com");
  const all = filledFor("reply_all", RECEIVED, "sam@example.com");

  it("keeps someone the member added, in the field they put them in", () => {
    const now = { to: [MAYA], cc: [BEA], bcc: [] };
    expect(carryRecipients(now, reply, all)).toEqual({ to: [MAYA, RUI], cc: [LEA, BEA], bcc: [] });
  });

  it("leaves out someone the member removed", () => {
    const now = { to: [MAYA, RUI], cc: [], bcc: [] };
    expect(carryRecipients(now, all, all)).toEqual({ to: [MAYA, RUI], cc: [], bcc: [] });
  });

  it("back to Reply drops the people Reply all filled in, not the ones the member added", () => {
    const now = { to: [MAYA, RUI], cc: [LEA, BEA], bcc: [] };
    expect(carryRecipients(now, all, reply)).toEqual({ to: [MAYA], cc: [BEA], bcc: [] });
  });

  it("to Forward keeps only the member's own, and Bcc stays hidden", () => {
    const now = { to: [MAYA], cc: [BEA], bcc: [RUI] };
    expect(carryRecipients(now, reply, none)).toEqual({ to: [], cc: [BEA], bcc: [RUI] });
    // Someone in Bcc stays only there, whatever the new kind fills in.
    expect(carryRecipients({ to: [], cc: [], bcc: [RUI] }, none, all)).toEqual({
      to: [MAYA],
      cc: [LEA],
      bcc: [RUI],
    });
  });

  it("names nobody twice, whatever the case of the address", () => {
    const now = { to: [MAYA], cc: [{ ...BEA, address: "BEA@example.com" }], bcc: [] };
    const after = { to: [MAYA, BEA], cc: [], bcc: [] };
    expect(carryRecipients(now, reply, after)).toEqual({ to: [MAYA, BEA], cc: [], bcc: [] });
  });
});
