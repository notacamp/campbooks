import { describe, expect, it } from "vitest";

import {
  BODY_IMAGES_STALE_MS,
  bodyRefetchInterval,
  bodyStaleTime,
  refetchBodyOnFocus,
  remoteImagesAllowed,
} from "./body-freshness";

const trusted = { html: "<p>Hi</p>", images_trusted: true };
const held = { html: "<p>Hi</p>", images_held: "rule_paused" };
const offered = { html: "<p>Hi</p>", images_offer: true, images_held: null };
const plain = { html: "<p>Hi</p>", images_trusted: false, images_held: null };

describe("a message body's freshness (campbooks-cloud#236)", () => {
  it("lasts until a live rule change on the web", () => {
    for (const body of [trusted, held, offered, plain]) {
      expect(bodyStaleTime(body, true)).toBe(Number.POSITIVE_INFINITY);
      expect(bodyRefetchInterval(body, true)).toBe(false);
      expect(refetchBodyOnFocus(body, true)).toBe(false);
    }
  });

  it("is read again when shown and every minute while open, without a live feed (Tauri), when a rule trusted it", () => {
    expect(bodyStaleTime(trusted, false)).toBe(0);
    expect(bodyRefetchInterval(trusted, false)).toBe(BODY_IMAGES_STALE_MS);
    expect(refetchBodyOnFocus(trusted, false)).toBe(true);
  });

  it("is a minute, and re-read on focus, for held or offered images without a live feed", () => {
    for (const body of [held, offered]) {
      expect(bodyStaleTime(body, false)).toBe(BODY_IMAGES_STALE_MS);
      expect(bodyRefetchInterval(body, false)).toBe(false);
      expect(refetchBodyOnFocus(body, false)).toBe(true);
    }
    expect(BODY_IMAGES_STALE_MS).toBe(60_000);
  });

  it("never expires for a body without remote-image state, even without a live feed", () => {
    for (const body of [plain, undefined, { content: "words only", truncated: false }]) {
      expect(bodyStaleTime(body, false)).toBe(Number.POSITIVE_INFINITY);
      expect(bodyRefetchInterval(body, false)).toBe(false);
      expect(refetchBodyOnFocus(body, false)).toBe(false);
    }
  });
});

describe("whether a body's remote images may load (campbooks-cloud#236)", () => {
  const shownByRule = { images_shown: true, images_trusted: true };
  const shownAsked = { images_shown: true, images_trusted: false };
  const notShown = { images_shown: false, images_trusted: false };
  const tauri = { live: false, showImages: false };

  it("never when the server held them back", () => {
    for (const context of [
      { ...tauri, openedAt: 0 },
      { live: true, showImages: true, openedAt: 0 },
    ]) {
      expect(remoteImagesAllowed({ data: notShown, dataUpdatedAt: 5 }, context)).toBe(false);
    }
  });

  it("holds a trusted body from the cache back, without a live feed, until it is read again", () => {
    // Read at 5, opened then: the cached trust may have been removed elsewhere.
    expect(
      remoteImagesAllowed({ data: shownByRule, dataUpdatedAt: 5 }, { ...tauri, openedAt: 5 }),
    ).toBe(false);
    // Read again since it opened: the rule still holds.
    expect(
      remoteImagesAllowed({ data: shownByRule, dataUpdatedAt: 9 }, { ...tauri, openedAt: 5 }),
    ).toBe(true);
    // Opened before its first read.
    expect(
      remoteImagesAllowed({ data: shownByRule, dataUpdatedAt: 9 }, { ...tauri, openedAt: 0 }),
    ).toBe(true);
    // Folded.
    expect(
      remoteImagesAllowed(
        { data: shownByRule, dataUpdatedAt: 9 },
        { ...tauri, openedAt: undefined },
      ),
    ).toBe(false);
  });

  it("lets the member's own Show images, and the live web, show them at once", () => {
    expect(
      remoteImagesAllowed({ data: shownAsked, dataUpdatedAt: 5 }, { ...tauri, openedAt: 5 }),
    ).toBe(true);
    expect(
      remoteImagesAllowed(
        { data: shownByRule, dataUpdatedAt: 5 },
        { live: false, showImages: true, openedAt: 5 },
      ),
    ).toBe(true);
    // The web drops a trusted body on every rule change instead.
    expect(
      remoteImagesAllowed(
        { data: shownByRule, dataUpdatedAt: 5 },
        { live: true, showImages: false, openedAt: 5 },
      ),
    ).toBe(true);
  });
});
