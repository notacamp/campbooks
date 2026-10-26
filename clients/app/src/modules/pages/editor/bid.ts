import { Extension } from "@tiptap/core";
import type { Node as ProseNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { ySyncPluginKey } from "@tiptap/y-tiptap";

/**
 * The blocks that carry a stable id (`bid`): the one a block keeps across
 * edits, moves and syncs, so Notion's version of it can be told from ours. The
 * server gives ids the same way (`crates/modules/pages/src/domain/notion/edit.rs`,
 * `assign_ids`): leaf blocks and the blocks that hold others, not a list's own
 * element, not an embed, not a conflict.
 */
export const BID_TYPES = [
  "paragraph",
  "heading",
  "codeBlock",
  "horizontalRule",
  "image",
  "file",
  "pageLink",
  "external",
  "listItem",
  "taskItem",
  "toggle",
  "blockquote",
  "callout",
] as const;

const BID_TYPE_SET: ReadonlySet<string> = new Set(BID_TYPES);

/** The blocks whose first paragraph is their own text, not a block of its own. */
const LEADING = new Set(["listItem", "taskItem", "toggle", "blockquote", "callout"]);

/** A block that needs a new id, and the one it gets. */
export interface IdFix {
  pos: number;
  bid: string;
}

/**
 * The blocks of `doc` that lack an id, or share one with an earlier block
 * (a split keeps the attributes of both halves), with fresh ones. Nothing
 * inside a conflict (it holds Notion's version, which is not ours to
 * number) and not the paragraph a block leads with.
 */
export function idFixes(doc: ProseNode, fresh: () => string): IdFix[] {
  const fixes: IdFix[] = [];
  const seen = new Set<string>();
  doc.descendants((node, pos, parent, index) => {
    const name = node.type.name;
    if (name === "conflict") {
      return false;
    }
    if (!BID_TYPE_SET.has(name)) {
      return true;
    }
    if (name === "paragraph" && index === 0 && parent !== null && LEADING.has(parent.type.name)) {
      return true;
    }
    const bid: unknown = node.attrs["bid"];
    if (typeof bid === "string" && bid !== "" && !seen.has(bid)) {
      seen.add(bid);
      return true;
    }
    const next = fresh();
    seen.add(next);
    fixes.push({ pos, bid: next });
    return true;
  });
  return fixes;
}

const attribute = {
  bid: {
    default: null,
    parseHTML: (element: HTMLElement) => element.dataset.bid,
    renderHTML: (attributes: Record<string, unknown>) => {
      const bid = attributes["bid"];
      return typeof bid === "string" ? { "data-bid": bid } : {};
    },
  },
};

/**
 * A stable id on every block, kept through the Yjs document, and given to a
 * block made here (or split from one) when it lacks one. Changes that come
 * from another client are left alone: whoever made the block numbers it, and
 * the server numbers what nobody did.
 */
export const Bid = Extension.create({
  name: "bid",
  addGlobalAttributes() {
    return [{ types: [...BID_TYPES], attributes: attribute }];
  },
  addProseMirrorPlugins() {
    const editor = this.editor;
    return [
      new Plugin({
        key: new PluginKey("bid"),
        appendTransaction: (transactions, _before, state) => {
          if (!editor.isEditable || !transactions.some((tr) => tr.docChanged)) {
            return null;
          }
          const remote = transactions.every(
            (tr) =>
              (tr.getMeta(ySyncPluginKey) as { isChangeOrigin?: boolean } | undefined)
                ?.isChangeOrigin === true,
          );
          if (remote) {
            return null;
          }
          const fixes = idFixes(state.doc, () => crypto.randomUUID());
          if (fixes.length === 0) {
            return null;
          }
          const tr = state.tr;
          for (const { pos, bid } of fixes) {
            const node = state.doc.nodeAt(pos);
            if (node !== null) {
              tr.setNodeMarkup(pos, undefined, { ...node.attrs, bid });
            }
          }
          // An id is not an edit: Undo does not step over it.
          tr.setMeta("addToHistory", false);
          return tr;
        },
      }),
    ];
  },
});
