import { Editor, getSchema } from "@tiptap/core";
import { TaskItem } from "@tiptap/extension-task-item";
import { TaskList } from "@tiptap/extension-task-list";
import { StarterKit } from "@tiptap/starter-kit";
import { ySyncPluginKey } from "@tiptap/y-tiptap";
import { describe, expect, it } from "vitest";

import { Bid, BID_TYPES, idFixes } from "./bid";
import { Callout, Conflict, External, Toggle } from "./nodes";

const extensions = [
  StarterKit.configure({ undoRedo: false }),
  TaskList,
  TaskItem.configure({ nested: true }),
  Callout,
  Toggle,
  External,
  Conflict,
  Bid,
];

const schema = getSchema(extensions);
const p = (text: string, bid?: string) => ({
  type: "paragraph",
  ...(bid === undefined ? {} : { attrs: { bid } }),
  content: [{ type: "text", text }],
});
const doc = (...content: object[]) => schema.nodeFromJSON({ type: "doc", content });

let counter = 0;
const fresh = () => {
  counter += 1;
  return `new-${String(counter)}`;
};

describe("idFixes", () => {
  it("gives an id to every block that lacks one, and leaves the ones that have", () => {
    const fixes = idFixes(doc(p("a", "keep"), p("b"), { type: "horizontalRule" }), fresh);
    expect(fixes).toHaveLength(2);
    expect(fixes.every((fix) => fix.bid.startsWith("new-"))).toBe(true);
  });

  it("gives a second block the same id as an earlier one a new id (a split keeps both halves' attributes)", () => {
    const fixes = idFixes(doc(p("a", "same"), p("b", "same")), fresh);
    expect(fixes).toHaveLength(1);
    expect(fixes[0]?.pos).toBeGreaterThan(0);
  });

  it("does not number the paragraph a block leads with, but its other blocks", () => {
    const toggle = {
      type: "toggle",
      content: [p("Title of the toggle"), p("Inside")],
    };
    const item = {
      type: "bulletList",
      content: [{ type: "listItem", content: [p("One")] }],
    };
    const fixes = idFixes(doc(toggle, item), fresh);
    // The toggle, the paragraph inside it, and the list item: not the two lead paragraphs, not the list.
    expect(fixes).toHaveLength(3);
  });

  it("does not number what is inside a conflict", () => {
    const conflict = { type: "conflict", attrs: { of: "x" }, content: [p("Notion’s version")] };
    expect(idFixes(doc(p("ours", "x"), conflict), fresh)).toHaveLength(0);
  });

  it("numbers a block kept in Notion", () => {
    expect(idFixes(doc({ type: "external", attrs: { kind: "column_list" } }), fresh)).toHaveLength(
      1,
    );
  });

  it("names the block types the server numbers", () => {
    expect(BID_TYPES).toContain("paragraph");
    expect(BID_TYPES).not.toContain("bulletList");
    expect(BID_TYPES).not.toContain("conflict");
  });
});

describe("the conflict and external nodes", () => {
  it("round-trip the attributes the server writes", () => {
    const node = schema.nodeFromJSON({
      type: "conflict",
      attrs: { of: "b", hash: "ab".repeat(32), edited: "2026-10-25T09:00:00.000Z", removed: true },
    });
    expect(node.attrs).toMatchObject({
      of: "b",
      removed: true,
      edited: "2026-10-25T09:00:00.000Z",
    });
    expect(node.content.childCount).toBe(0);
    const kept = schema.nodeFromJSON({ type: "external", attrs: { kind: "synced_block" } });
    expect(kept.attrs["kind"]).toBe("synced_block");
    expect(kept.isAtom).toBe(true);
  });
});

describe("the Bid extension", () => {
  const make = (editable = true) =>
    new Editor({ extensions, editable, content: { type: "doc", content: [p("first", "one")] } });

  const bids = (editor: Editor) => {
    const found: unknown[] = [];
    editor.state.doc.descendants((node) => {
      if (BID_TYPES.includes(node.type.name as (typeof BID_TYPES)[number])) {
        found.push(node.attrs["bid"]);
      }
    });
    return found;
  };

  it("gives a block made here an id, and keeps the ones there", () => {
    const editor = make();
    editor.commands.insertContentAt(editor.state.doc.content.size, p("second"));
    const found = bids(editor);
    expect(found).toHaveLength(2);
    expect(found[0]).toBe("one");
    expect(typeof found[1]).toBe("string");
    expect(found[1]).not.toBe("one");
    editor.destroy();
  });

  it("numbers a block split from another, not twice with one id", () => {
    const editor = make();
    editor.commands.setTextSelection(3);
    editor.commands.splitBlock();
    const found = bids(editor);
    expect(new Set(found).size).toBe(found.length);
    editor.destroy();
  });

  it("leaves a read-only page alone", () => {
    const editor = make(false);
    editor.view.dispatch(
      editor.state.tr.insert(editor.state.doc.content.size, schema.nodeFromJSON(p("x"))),
    );
    expect(bids(editor)).toEqual(["one", null]);
    editor.destroy();
  });

  it("leaves alone what another client changed", () => {
    const editor = make();
    editor.view.dispatch(
      editor.state.tr
        .insert(editor.state.doc.content.size, schema.nodeFromJSON(p("remote")))
        .setMeta(ySyncPluginKey, { isChangeOrigin: true }),
    );
    expect(bids(editor)).toEqual(["one", null]);
    editor.destroy();
  });
});
