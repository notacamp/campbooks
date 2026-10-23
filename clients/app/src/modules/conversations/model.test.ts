import { describe, expect, it } from "vitest";

import {
  FIGURES,
  LEDGER,
  NOW,
  Q4,
  Q4_DETAIL,
  SENT,
  SPAMMED,
} from "../../../test/demo/conversations";

import {
  applied,
  belongsIn,
  isPlace,
  isView,
  STREAMS,
  VIEWS,
  type MessageView,
} from "./api/queries";
import {
  addresseeLine,
  archiveMove,
  attachmentKind,
  cleanFileName,
  displayName,
  listTime,
  noteSentence,
  openByDefault,
  placeName,
  placementLine,
  previewType,
  reasonText,
  replyName,
  sentToLine,
  spamMove,
  splitQuoted,
  suggestsReply,
  threadLayout,
  trashMove,
  undoMoves,
  withLeaving,
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
    expect(suggestsReply(Q4_DETAIL.decisions)).toBe(true);
    expect(suggestsReply([])).toBe(false);
    expect(
      suggestsReply(
        Q4_DETAIL.decisions.map((d) => (d.question === "needs_reply" ? { ...d, value: false } : d)),
      ),
    ).toBe(false);
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

  it("tells the views from the streams: a place is either, nothing else", () => {
    expect(VIEWS.every((view) => isView(view) && isPlace(view))).toBe(true);
    expect(STREAMS.some((stream) => isView(stream))).toBe(false);
    expect(STREAMS.every((stream) => isPlace(stream))).toBe(true);
    for (const junk of ["inbox", "Starred", "Sent", "pinned", ""]) {
      expect(isPlace(junk)).toBe(false);
    }
    // Pinned is a view; Drafts is a place whose list is not conversations.
    expect(isPlace("starred") && isView("starred")).toBe(true);
    expect(isPlace("drafts")).toBe(true);
    expect(isView("drafts")).toBe(false);
    expect(placeName(t, "starred")).toBe("views.starred");
    expect(placeName(t, "drafts")).toBe("views.drafts");
    expect(placeName(t, "trash")).toBe("views.trash");
    expect(placeName(t, "primary")).toBe("streams.primary");
  });

  it("leads a Sent row with who the inbox wrote to", () => {
    expect(sentToLine(t, FIGURES)).toBe('list.toOthers{"name":"Marta Ferreira","count":2}');
    expect(sentToLine(t, Q4)).toBe('list.to{"name":"Maya Okafor"}');
    expect(sentToLine(t, { ...Q4, sent_to: [] })).toBe("list.toHidden");
    expect(SENT.map((summary) => summary.replied_since_sent)).toEqual([false, true]);
  });

  it("lists a conversation where the server does, before and after each move", () => {
    const where = (summary: typeof Q4) =>
      [...STREAMS, ...VIEWS].filter((place) => belongsIn(place, summary));
    expect(where(Q4)).toEqual(["primary", "sent"]);
    expect(where(FIGURES)).toEqual(["sent", "archive"]);
    expect(where(SPAMMED)).toEqual(["spam"]);
    // Delete leaves every list but Trash; Restore puts it back where it was.
    expect(where(applied(Q4, "trash"))).toEqual(["trash"]);
    expect(where(applied(applied(Q4, "trash"), "untrash"))).toEqual(["primary", "sent"]);
    expect(where(applied(FIGURES, "trash"))).toEqual(["trash"]);
    expect(where(applied(applied(FIGURES, "trash"), "untrash"))).toEqual(["sent", "archive"]);
    // Report spam: Spam only; Not spam: back in the inbox.
    expect(where(applied(Q4, "spam"))).toEqual(["spam"]);
    expect(where(applied(SPAMMED, "not_spam"))).toEqual(["primary"]);
    // Done and Move to Inbox.
    expect(where(applied(Q4, "archive"))).toEqual(["sent", "archive"]);
    expect(where(applied(FIGURES, "unarchive"))).toEqual(["primary", "sent"]);
  });

  it("lists a pinned conversation in Pinned, archived or not, and never from the trash", () => {
    const pinned = { ...Q4, starred: true };
    const where = (summary: typeof Q4) => VIEWS.filter((view) => belongsIn(view, summary));
    expect(where(Q4)).toEqual(["sent"]);
    expect(where(pinned)).toEqual(["sent", "starred"]);
    expect(where(applied(pinned, "archive"))).toEqual(["sent", "starred", "archive"]);
    expect(where(applied(pinned, "trash"))).toEqual(["trash"]);
    expect(where(applied(applied(pinned, "trash"), "untrash"))).toEqual(["sent", "starred"]);
    expect(where(applied(pinned, "spam"))).toEqual(["spam"]);
    // Pin and Unpin move it in and out of Pinned and nowhere else.
    expect(where(applied(Q4, "star"))).toEqual(["sent", "starred"]);
    expect(where(applied(pinned, "unstar"))).toEqual(["sent"]);
  });

  it("gives each key the move that fits where the conversation is", () => {
    const binned = { ...Q4, trashed: true, spam: false };
    expect([archiveMove(Q4), trashMove(Q4), spamMove(Q4)]).toEqual(["archive", "trash", "spam"]);
    expect(archiveMove(FIGURES)).toBe("unarchive");
    // In spam: ! says it is not; # and E do nothing (Delete would clear the spam mark).
    expect([archiveMove(SPAMMED), trashMove(SPAMMED), spamMove(SPAMMED)]).toEqual([
      undefined,
      undefined,
      "not_spam",
    ]);
    // In the trash: # restores it; nothing else.
    expect([archiveMove(binned), trashMove(binned), spamMove(binned)]).toEqual([
      undefined,
      "untrash",
      undefined,
    ]);
  });

  it("undoes Report spam back where it was: archived again when it was archived", () => {
    expect(undoMoves(FIGURES, "spam")).toEqual(["not_spam", "archive"]);
    expect(undoMoves(Q4, "spam")).toEqual(["not_spam"]);
    // The plain Not spam stays one step, back to the inbox.
    expect(undoMoves(SPAMMED, "not_spam")).toEqual(["spam"]);
    expect(undoMoves(Q4, "trash")).toEqual(["untrash"]);
    expect(undoMoves(FIGURES, "unarchive")).toEqual(["archive"]);
    let undone = applied(FIGURES, "spam");
    for (const move of undoMoves(FIGURES, "spam")) {
      undone = applied(undone, move);
    }
    expect([undone.archived, undone.trashed, undone.spam]).toEqual([true, false, false]);
    // Were spam ever deleted, Undo marks it spam again.
    expect(undoMoves(SPAMMED, "trash")).toEqual(["untrash", "spam"]);
  });
});

describe("withLeaving", () => {
  const row = (id: string) => ({ ...Q4, id });
  const a = row("a");
  const b = row("b");
  const c = row("c");
  const d = row("d");
  const ids = (rows: readonly { id: string }[]) => rows.map((summary) => summary.id).join("");

  it("keeps each leaving row above the row it sat above, as rows go one after the other", () => {
    // E, E from the top: a left from above b, then b from above c.
    const leaving = [
      { conversation: a, before: "b", at: 0 },
      { conversation: b, before: "c", at: 1 },
    ];
    expect(ids(withLeaving([c, d], leaving))).toBe("abcd");
    // a has folded away: b is still above c.
    expect(ids(withLeaving([c, d], leaving.slice(1)))).toBe("bcd");
  });

  it("puts the last row back last, and a row above it before it", () => {
    const leaving = [
      { conversation: d, before: undefined, at: 3 },
      { conversation: c, before: "d", at: 2 },
    ];
    expect(ids(withLeaving([a, b], leaving))).toBe("abcd");
  });

  it("falls back to where it was when the row below is gone", () => {
    expect(ids(withLeaving([a, c, d], [{ conversation: b, before: "x", at: 1 }]))).toBe("abcd");
    expect(ids(withLeaving([a], [{ conversation: b, before: "x", at: 5 }]))).toBe("ab");
  });

  it("drops a leaving row that is back on the list (Undo while it folded)", () => {
    expect(ids(withLeaving([a, b, c], [{ conversation: b, before: "c", at: 1 }]))).toBe("abc");
  });
});
