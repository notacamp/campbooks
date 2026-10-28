import { describe, expect, it } from "vitest";

import { fromMarks, isLinkable, normalizeAddress, toMarks, type DocNode } from "./marks";

const text = (value: string, ...marks: string[]): DocNode => ({
  type: "text",
  text: value,
  ...(marks.length === 0 ? {} : { marks: marks.map((type) => ({ type })) }),
});
const doc = (...content: DocNode[]): DocNode => ({ type: "doc", content });
const p = (...content: DocNode[]): DocNode => ({ type: "paragraph", content });

describe("marks", () => {
  it("reads an empty draft as one empty paragraph, and writes it back empty", () => {
    expect(fromMarks("")).toEqual({ type: "doc", content: [{ type: "paragraph" }] });
    expect(toMarks(fromMarks(""))).toBe("");
  });

  it("round-trips what the server reads", () => {
    for (const marks of [
      "Hi **Ana**,\n\n- one\n- two\n\nThanks *a lot*.",
      "> quoted\n>\n> - a\n> - b\n\nafter",
      "1. a\n2. b\n\n[the lease](https://example.com/4b) and ~~gone~~",
      "2 \\* 3 \\[x\\] C:\\\\temp",
    ]) {
      expect(toMarks(fromMarks(marks))).toBe(marks);
    }
  });

  it("keeps a space at the edge of a mark outside it", () => {
    expect(toMarks(doc(p(text("a"), text(" b ", "italic"), text("c"))))).toBe("a *b* c");
    expect(toMarks(doc(p(text(" ", "bold"))))).toBe(" ");
  });

  it("writes a typed address as a bare link, and one with words in brackets", () => {
    const link = (words: string, href: string): DocNode => ({
      type: "text",
      text: words,
      marks: [{ type: "link", attrs: { href } }],
    });
    expect(toMarks(doc(p(link("https://example.com/a", "https://example.com/a"))))).toBe(
      "https://example.com/a",
    );
    expect(toMarks(doc(p(link("the lease", "https://example.com/a(1)"))))).toBe(
      "[the lease](https://example.com/a(1%29)",
    );
    // A link the server would refuse is only its words.
    expect(toMarks(doc(p(link("click", "javascript:alert(1)"))))).toBe("click");
  });

  it("flattens a list nested deeper than one level, and a heading to a paragraph", () => {
    const item = (...content: DocNode[]): DocNode => ({ type: "listItem", content });
    const nested: DocNode = {
      type: "bulletList",
      content: [
        item(p(text("a")), {
          type: "bulletList",
          content: [item(p(text("b")))],
        }),
      ],
    };
    expect(toMarks(doc(nested))).toBe("- a\n- b");
    expect(toMarks(doc({ type: "heading", content: [text("Title")] }))).toBe("Title");
  });

  it("only accepts web and mail addresses", () => {
    expect(isLinkable("https://example.com")).toBe(true);
    expect(isLinkable("mailto:ana@example.com")).toBe(true);
    expect(isLinkable("javascript:alert(1)")).toBe(false);
    expect(isLinkable("https://")).toBe(false);
    expect(isLinkable("https://a b")).toBe(false);
    expect(normalizeAddress(" example.com/x ")).toBe("https://example.com/x");
    expect(normalizeAddress("ana@example.com")).toBe("mailto:ana@example.com");
    expect(normalizeAddress("javascript:alert(1)")).toBe("javascript:alert(1)");
  });
});
