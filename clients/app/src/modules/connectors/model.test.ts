import { describe, expect, it } from "vitest";

import { CONNECTION } from "../../../test/demo/identity";

import { firstChoice, healthTone, mailboxResult, stepChoice } from "./model";

const zoho = {
  ...CONNECTION,
  id: "c2",
  provider: "zoho" as const,
  health: "needs_reauth" as const,
};
const other = {
  ...CONNECTION,
  id: "c3",
  granted_by: "someone-else",
  health: "needs_reauth" as const,
};

describe("connectors model", () => {
  it("reads what the front door came back with", () => {
    expect(mailboxResult({ connected: "c1" })).toEqual({ kind: "connected", connection: "c1" });
    expect(mailboxResult({ link_error: "account_mismatch" })).toEqual({
      kind: "error",
      error: "account_mismatch",
    });
    expect(mailboxResult({ link_error: "made_up" })).toEqual({ kind: "error", error: "unknown" });
    expect(mailboxResult({})).toBeUndefined();
  });

  it("gives every health a tone", () => {
    expect(healthTone("ok")).toBe("ok");
    expect(healthTone("needs_reauth")).toBe("problem");
    expect(healthTone("throttled")).toBe("waiting");
    expect(healthTone("failing")).toBe("waiting");
  });

  it("chooses first the mailbox of yours that needs you", () => {
    const mine = (c: { granted_by: string }) => c.granted_by === CONNECTION.granted_by;
    expect(firstChoice([CONNECTION, other, zoho], mine)).toBe("c2");
    expect(firstChoice([CONNECTION, other], mine)).toBe(CONNECTION.id);
    expect(firstChoice([], mine)).toBeUndefined();
  });

  it("moves the choice within the list", () => {
    const list = [CONNECTION, zoho, other];
    expect(stepChoice(list, CONNECTION.id, 1)).toBe("c2");
    expect(stepChoice(list, "c3", 1)).toBe("c3");
    expect(stepChoice(list, CONNECTION.id, -1)).toBe(CONNECTION.id);
    expect(stepChoice(list, undefined, 1)).toBe(CONNECTION.id);
    expect(stepChoice([], undefined, 1)).toBeUndefined();
  });
});
