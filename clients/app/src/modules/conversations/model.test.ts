import { describe, expect, it } from "vitest";

import { LEDGER, NOW, Q4, Q4_DETAIL } from "../../../test/demo/conversations";

import {
  attachmentKind,
  cleanFileName,
  displayName,
  listTime,
  noteSentence,
  openByDefault,
  placementLine,
  previewType,
  reasonText,
  type Translate,
} from "./model";

/** A `t` that shows the key and its options, to test what is asked for. */
const t: Translate = (key, options) =>
  options === undefined ? key : `${key}${JSON.stringify(options)}`;
const exists = (key: string) => key !== "reasons.unknown.rule";

describe("inbox model", () => {
  it("formats list times relative to now", () => {
    expect(listTime("2026-10-13T09:05:00Z", "en-GB", NOW)).toMatch(/\d{2}:05/u);
    expect(listTime("2026-10-10T09:05:00Z", "en-GB", NOW)).toBe("Sat");
    expect(listTime("2026-09-02T09:05:00Z", "en-GB", NOW)).toBe("2 Sept");
    expect(listTime("2025-09-02T09:05:00Z", "en-GB", NOW)).toBe("2 Sept 2025");
  });

  it("names a sender by display name, else by address", () => {
    expect(displayName({ name: "Maya Okafor", address: "maya@example.com" })).toBe("Maya Okafor");
    expect(displayName({ name: "  ", address: "maya@example.com" })).toBe("maya@example.com");
    expect(displayName(null)).toBe("");
  });

  it("says the decide answers as suggestions, and nothing when there are none", () => {
    expect(noteSentence(t, Q4_DETAIL.decisions, "en")).toBe(
      'scout.read{"list":"scout.needsReply and scout.hasDeadline"}',
    );
    expect(noteSentence(t, [], "en")).toBe("scout.nothing");
  });

  it("puts a suggestion before where the conversation is", () => {
    // In words: no percentage up front.
    expect(placementLine(t, LEDGER)).toBe('scout.suggestion{"stream":"streams.newsletters"}');
    expect(placementLine(t, Q4)).toBe('scout.sortedDefault{"stream":"streams.primary"}');
    expect(placementLine(t, { ...Q4, stream_source: "member" })).toBe(
      'scout.sortedMember{"stream":"streams.primary"}',
    );
  });

  it("explains with the stored reasons, translated, and unknown rules as they are", () => {
    expect(reasonText(t, exists, { rule: "signals.deadline", evidence: "Friday" })).toBe(
      'reasons.signals.deadline{"evidence":"Friday"}',
    );
    expect(reasonText(t, exists, { rule: "unknown.rule", evidence: "x" })).toBe(
      'reasons.other{"rule":"unknown.rule","evidence":"x"}',
    );
  });

  it("opens the newest message and the unread ones", () => {
    const open = openByDefault(Q4_DETAIL.messages);
    expect([...open]).toEqual([Q4_DETAIL.messages[2]?.id]);
  });
});

describe("attachments", () => {
  it("previews raster images and PDFs only", () => {
    expect(attachmentKind("image/jpeg")).toBe("image");
    expect(attachmentKind("IMAGE/PNG; name=a.png")).toBe("image");
    expect(attachmentKind("application/pdf")).toBe("pdf");
    for (const type of ["text/html", "image/svg+xml", "application/xhtml+xml", "text/plain", ""]) {
      expect(attachmentKind(type)).toBe("file");
      expect(previewType(type)).toBeUndefined();
    }
    expect(previewType("image/webp; x=1")).toBe("image/webp");
  });

  it("cleans file names of controls, bidi overrides and separators", () => {
    expect(cleanFileName("invoice‮fdp.exe")).toBe("invoicefdp.exe");
    expect(cleanFileName("../../etc/pass\r\nwd")).toBe("etcpasswd");
    expect(cleanFileName(" .hidden ")).toBe("hidden");
    expect(cleanFileName("\u0000⁦")).toBe("attachment");
    expect(cleanFileName("orçamento 2026.pdf")).toBe("orçamento 2026.pdf");
  });
});
