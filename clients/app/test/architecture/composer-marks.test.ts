// @vitest-environment node
import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import {
  fromMarks,
  toMarks,
  type DocNode,
} from "../../src/lib/ui/components/MessageComposer/marks";

interface Case {
  name: string;
  doc: DocNode;
  marks: string;
  html: string;
  plain: string;
}

// The server's cases (`conversations`' `domain/format.rs` reads the same
// file): the document and the marks it is saved as must agree on both sides.
const cases = JSON.parse(
  readFileSync(
    new URL("../../../../crates/modules/conversations/tests/fixtures/marks.json", import.meta.url),
    "utf8",
  ),
) as Case[];

describe("the composer's marks, against the server's fixture", () => {
  it("has the cases", () => {
    expect(cases.length).toBeGreaterThan(20);
  });

  it.each(cases.map((c) => [c.name, c] as const))("%s: document to marks", (_name, c) => {
    expect(toMarks(c.doc)).toBe(c.marks);
  });

  it.each(cases.map((c) => [c.name, c] as const))("%s: marks to document", (_name, c) => {
    expect(fromMarks(c.marks)).toEqual(c.doc);
  });
});
