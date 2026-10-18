// @vitest-environment node
import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { FREE_MAIL } from "../../src/lib/mail/free-mail";

// Where the list is defined: the taught rules engine (`FREE_MAIL`), outside
// the app's root, so it is read from disk rather than imported.
const taught = readFileSync(
  new URL("../../../../crates/ai/src/decide/taught.rs", import.meta.url),
  "utf8",
);

describe("the web client's free mail list", () => {
  it("is the server's list, entry for entry", () => {
    const block = /pub const FREE_MAIL: &\[&str\] = &\[([\s\S]*?)\];/u.exec(taught)?.[1] ?? "";
    const server = [...block.matchAll(/"([^"]+)"/gu)].map((match) => match[1]);
    expect(server.length).toBeGreaterThan(300);
    expect([...FREE_MAIL].toSorted()).toEqual(server.toSorted());
  });
});
