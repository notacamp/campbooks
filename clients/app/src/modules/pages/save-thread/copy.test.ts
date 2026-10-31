import { describe, expect, it } from "vitest";

import { buildCopy, sanitiseText } from "./copy";

const day = (iso: string) => iso.slice(0, 10);

describe("buildCopy", () => {
  it("writes a small heading, 'Sender — date', then the words as paragraphs", () => {
    const blocks = buildCopy(
      [
        {
          from: "Ana Duarte",
          sentAt: "2026-10-21T09:00:00Z",
          text: "Hi Rui,\nthree things.\n\nThanks",
        },
        { from: "Rui Costa", sentAt: "2026-10-22T10:00:00Z", text: "Done." },
      ],
      day,
    );
    expect(blocks.map((block) => block.type)).toEqual([
      "heading",
      "paragraph",
      "paragraph",
      "heading",
      "paragraph",
    ]);
    expect(blocks[0]).toEqual({
      type: "heading",
      attrs: { level: 3 },
      content: [{ type: "text", text: "Ana Duarte — 2026-10-21" }],
    });
    // A single line break stays a break inside the paragraph.
    expect(blocks[1]?.content).toEqual([
      { type: "text", text: "Hi Rui," },
      { type: "hardBreak" },
      { type: "text", text: "three things." },
    ]);
  });

  it("leaves a message with no words as its heading alone", () => {
    expect(
      buildCopy([{ from: "Ana", sentAt: "2026-10-21T09:00:00Z", text: " \n " }], day),
    ).toHaveLength(1);
  });

  it("only ever writes text: nothing in it is markup, a link or a mention", () => {
    const blocks = buildCopy(
      [
        {
          from: "Ana",
          sentAt: "2026-10-21T09:00:00Z",
          text: "<b>hi</b> [x](javascript:alert(1)) @rui",
        },
      ],
      day,
    );
    const json = JSON.stringify(blocks);
    expect(json).not.toContain("marks");
    expect(json).not.toContain("mention");
    expect(blocks[1]?.content).toEqual([
      { type: "text", text: "<b>hi</b> [x](javascript:alert(1)) @rui" },
    ]);
  });
});

describe("sanitiseText", () => {
  it("drops control characters and bidirectional overrides but keeps line breaks and emoji joiners", () => {
    expect(sanitiseText("a\u0000b‮c​d﻿e\r\nf\tg")).toBe("abcde\nf g");
    expect(sanitiseText("👩‍💻")).toBe("👩‍💻");
  });

  it("collapses runs of blank lines and trailing spaces", () => {
    expect(sanitiseText("a  \n\n\n\n b\n")).toBe("a\n\n b");
  });
});
