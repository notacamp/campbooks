import { describe, expect, it } from "vitest";

import { LEDGER, NOW, Q4, Q4_DETAIL } from "../../../test/demo/conversations";

import type { MessageView } from "./api/queries";
import {
  addresseeLine,
  attachmentKind,
  cleanFileName,
  displayName,
  listTime,
  noteSentence,
  openByDefault,
  placementLine,
  previewType,
  reasonText,
  replyName,
  splitQuoted,
  threadLayout,
  type ThreadEntry,
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

/** A thread of `count` read messages, oldest first: m1 … m<count>. */
function thread(count: number, unread: number[] = []): MessageView[] {
  const [first] = Q4_DETAIL.messages;
  if (first === undefined) {
    throw new Error("no fixture message");
  }
  return Array.from({ length: count }, (_, index) => ({
    ...first,
    id: `m${String(index + 1)}`,
    read: !unread.includes(index + 1),
  }));
}

/** The layout as short labels: "m7", or "+m5,m4" for a fold. */
function labels(entries: ThreadEntry[]): string[] {
  return entries.map((entry) =>
    entry.kind === "more" ? `+${entry.ids.join(",")}` : entry.message.id,
  );
}

describe("thread layout", () => {
  it("never folds a thread of four or fewer", () => {
    expect(labels(threadLayout(thread(3), new Set()))).toEqual(["m3", "m2", "m1"]);
    expect(labels(threadLayout(thread(4), new Set()))).toEqual(["m4", "m3", "m2", "m1"]);
  });

  it("keeps the newest two and the first, and folds the middle newest first", () => {
    expect(labels(threadLayout(thread(7), new Set()))).toEqual(["m7", "m6", "+m5,m4,m3,m2", "m1"]);
  });

  it("keeps unread and opened messages, and a single hidden one as itself", () => {
    expect(labels(threadLayout(thread(7, [4]), new Set()))).toEqual([
      "m7",
      "m6",
      "m5",
      "m4",
      "+m3,m2",
      "m1",
    ]);
    expect(labels(threadLayout(thread(7), new Set(["m3", "m4", "m5"])))).toEqual([
      "m7",
      "m6",
      "m5",
      "m4",
      "m3",
      "m2",
      "m1",
    ]);
  });

  it("lays out nothing for no messages", () => {
    expect(threadLayout([], new Set())).toEqual([]);
  });
});

describe("your messages", () => {
  const [maya, sams] = Q4_DETAIL.messages;

  it("say who they went to", () => {
    if (sams === undefined) {
      throw new Error("no fixture message");
    }
    expect(addresseeLine(t, sams)).toBe('reader.toName{"name":"Maya Okafor"}');
    expect(
      addresseeLine(t, { ...sams, cc: [{ name: "Leo Brandt", address: "leo@example.com" }] }),
    ).toBe('reader.toNameOthers{"name":"Maya Okafor","count":1}');
    expect(addresseeLine(t, { ...sams, to: [] })).toBe("");
  });

  it("answer their recipient; anyone else's answer the sender", () => {
    if (maya === undefined || sams === undefined) {
      throw new Error("no fixture message");
    }
    expect(replyName(sams)).toBe("Maya Okafor");
    expect(replyName(maya)).toBe("Maya Okafor");
  });
});

describe("quoted plain text", () => {
  it("folds the trailing run of quoted lines, attribution left as words", () => {
    expect(splitQuoted("Sounds good.\n\nOn Tue, Ana wrote:\n> The old text\n>\n> More\n")).toEqual({
      own: "Sounds good.\n\nOn Tue, Ana wrote:",
      quoted: "> The old text\n>\n> More",
    });
    expect(splitQuoted("Yes.\n\n> a\n\n> b\n\n")).toEqual({ own: "Yes.", quoted: "> a\n\n> b" });
  });

  it("leaves an inline reply, a bare quote and unquoted text whole", () => {
    const inline = "> Thursday?\nYes.\n> Oak?\nOak, please.";
    expect(splitQuoted(inline)).toEqual({ own: inline, quoted: undefined });
    expect(splitQuoted("> only a quote")).toEqual({ own: "> only a quote", quoted: undefined });
    expect(splitQuoted("Just words.")).toEqual({ own: "Just words.", quoted: undefined });
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
