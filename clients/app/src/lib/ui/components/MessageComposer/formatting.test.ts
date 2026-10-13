import { describe, expect, it } from "vitest";

import { applyFormat } from "./formatting";

describe("applyFormat", () => {
  it("wraps and unwraps the selection", () => {
    const bold = applyFormat({ text: "see you Thursday", start: 8, end: 16 }, "bold");
    expect(bold).toEqual({ text: "see you **Thursday**", start: 10, end: 18 });
    expect(applyFormat(bold, "bold")).toEqual({ text: "see you Thursday", start: 8, end: 16 });
    expect(applyFormat({ text: "", start: 0, end: 0 }, "italic")).toEqual({
      text: "**",
      start: 1,
      end: 1,
    });
  });

  it("turns the selected lines into a list", () => {
    const text = "Agenda:\nbudget\ntimeline";
    expect(applyFormat({ text, start: 8, end: text.length }, "bullets")).toEqual({
      text: "Agenda:\n- budget\n- timeline",
      start: 10,
      end: 27,
    });
    expect(applyFormat({ text, start: 9, end: 9 }, "numbers")).toEqual({
      text: "Agenda:\n1. budget\ntimeline",
      start: 12,
      end: 12,
    });
  });
});
