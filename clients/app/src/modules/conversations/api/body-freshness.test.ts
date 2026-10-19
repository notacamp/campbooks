import { describe, expect, it } from "vitest";

import { BODY_IMAGES_STALE_MS, bodyStaleTime, refetchBodyOnFocus } from "./body-freshness";

const trusted = { html: "<p>Hi</p>", images_trusted: true };
const held = { html: "<p>Hi</p>", images_held: "rule_paused" };
const offered = { html: "<p>Hi</p>", images_offer: true, images_held: null };
const plain = { html: "<p>Hi</p>", images_trusted: false, images_held: null };

describe("a message body's freshness (campbooks-cloud#236)", () => {
  it("lasts until a live rule change on the web", () => {
    for (const body of [trusted, held, offered, plain]) {
      expect(bodyStaleTime(body, true)).toBe(Number.POSITIVE_INFINITY);
      expect(refetchBodyOnFocus(body, true)).toBe(false);
    }
  });

  it("is a minute, and re-read on focus, for bodies whose images depend on rules without a live feed (Tauri)", () => {
    for (const body of [trusted, held, offered]) {
      expect(bodyStaleTime(body, false)).toBe(BODY_IMAGES_STALE_MS);
      expect(refetchBodyOnFocus(body, false)).toBe(true);
    }
    expect(BODY_IMAGES_STALE_MS).toBe(60_000);
  });

  it("never expires for a body without remote-image state, even without a live feed", () => {
    for (const body of [plain, undefined, { content: "words only", truncated: false }]) {
      expect(bodyStaleTime(body, false)).toBe(Number.POSITIVE_INFINITY);
      expect(refetchBodyOnFocus(body, false)).toBe(false);
    }
  });
});
