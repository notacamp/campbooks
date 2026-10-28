import { getSchema } from "@tiptap/core";
import { prosemirrorJSONToYXmlFragment } from "@tiptap/y-tiptap";
import * as Y from "yjs";

import { createExtensions } from "../editor/extensions";
import type { AttachedFile } from "../types";

/** A node of a page's body, as the editor's schema writes it (JSON). */
export interface BlockJson {
  type: string;
  attrs?: Record<string, unknown>;
  content?: BlockJson[];
  text?: string;
}

/** The field of the Yjs document the page's body lives in (the editor's own). */
const FIELD = "default";

/** The conversation, as a block: always the newest message, never a copy. */
export function threadBlock(conversationId: string): BlockJson {
  return {
    type: "embed",
    attrs: { ref_type: "conversations.conversation", ref_id: conversationId },
  };
}

const IMAGE_TYPES = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);

/** A file the page now holds, as a block: an image card for pictures, a file card otherwise. */
export function fileBlock(file: AttachedFile): BlockJson {
  return {
    type: IMAGE_TYPES.has(file.content_type) ? "image" : "file",
    attrs: { blob: file.blob, name: file.name, size: file.size },
  };
}

let schema: ReturnType<typeof getSchema> | undefined;

/** The page's own schema, made once: the same blocks the editor knows, no more. */
function pageSchema() {
  schema ??= getSchema(
    createExtensions({
      doc: new Y.Doc(),
      placeholders: { paragraph: "", heading: "" },
      menus: [],
    }),
  );
  return schema;
}

/**
 * A top-level block as text, to tell it from its neighbours: its markup
 * without the stable `bid` the editor or the server gives it afterwards.
 */
function signatureOf(node: Y.XmlElement): string {
  return node.toJSON().replaceAll(/ bid="[^"]*"/gu, "");
}

/**
 * Add `blocks` at the end of the page's body in `doc`, the way the editor
 * would have typed them (so the server merges them like any edit). Returns
 * what was added, one signature per top-level block, for taking it back.
 */
export function appendBlocks(doc: Y.Doc, blocks: readonly BlockJson[]): string[] {
  const scratch = new Y.Doc();
  const made = scratch.getXmlFragment(FIELD);
  prosemirrorJSONToYXmlFragment(pageSchema(), { type: "doc", content: [...blocks] }, made);
  const nodes = made.slice(0).filter((node): node is Y.XmlElement => node instanceof Y.XmlElement);
  const signatures = nodes.map(signatureOf);
  const body = doc.getXmlFragment(FIELD);
  doc.transact(() => {
    body.insert(
      body.length,
      nodes.map((node) => node.clone()),
    );
  });
  return signatures;
}

/**
 * Take back what {@link appendBlocks} added: for each signature, the last
 * top-level block that still reads the same. A block someone changed since
 * stays (it is theirs now). Returns how many were removed.
 */
export function removeBlocks(doc: Y.Doc, signatures: readonly string[]): number {
  const body = doc.getXmlFragment(FIELD);
  const wanted = [...signatures];
  const doomed: number[] = [];
  const present = body.slice(0);
  for (let index = present.length - 1; index >= 0; index -= 1) {
    const node = present[index];
    if (!(node instanceof Y.XmlElement)) {
      continue;
    }
    const at = wanted.indexOf(signatureOf(node));
    if (at !== -1) {
      wanted.splice(at, 1);
      doomed.push(index);
    }
  }
  doc.transact(() => {
    // From the end, so earlier positions hold.
    for (const index of doomed) {
      body.delete(index, 1);
    }
  });
  return doomed.length;
}
