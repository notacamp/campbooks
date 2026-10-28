import * as Y from "yjs";

import { fromBase64, toBase64 } from "../editor/bytes";
import type { AttachedFile, PageView, SyncResponse } from "../types";

import { appendBlocks, fileBlock, removeBlocks, type BlockJson } from "./blocks";
import { refusalOf, tooLarge, type Includable, type Refusal } from "./refusals";

/** A clean-up that fails has nothing more to take back. */
const noop = (): void => {
  // Nothing more to take back.
};

/** The calls saving a thread makes; the dialog passes the real ones, tests a fake server. */
export interface SaveCalls {
  read: (pageId: string) => Promise<PageView>;
  sync: (pageId: string, body: { state_vector: string; update?: string }) => Promise<SyncResponse>;
  attach: (
    pageId: string,
    conversation: string,
    message: string,
    attachments: readonly string[],
  ) => Promise<AttachedFile[]>;
  detach: (pageId: string, blobs: readonly string[]) => Promise<void>;
}

/** The attachments of one message to include. */
export interface MessageFiles {
  message: string;
  files: (Includable & { id: string })[];
}

export interface SaveRequest {
  pageId: string;
  conversation: string;
  /** The blocks to add: the live thread block, or the copy. */
  blocks: readonly BlockJson[];
  /** Attachments to make files of the page, by message. */
  attachments: readonly MessageFiles[];
}

/** What was added, so it can be taken back. */
export interface Saved {
  pageId: string;
  /** One signature per block added (see {@link removeBlocks}). */
  signatures: string[];
  /** The files the page now holds because of this save. */
  blobs: string[];
}

/** The save was refused for a reason the member can be told; nothing was added. */
export class SaveRefused extends Error {
  readonly refusal: Refusal;

  constructor(refusal: Refusal) {
    super(`Saving to the page was refused: ${refusal.reason}`);
    this.name = "SaveRefused";
    this.refusal = refusal;
  }
}

/** Send the changes `doc` holds beyond what the server had (`server`, a state vector). */
async function send(calls: SaveCalls, pageId: string, doc: Y.Doc, server: string) {
  await calls.sync(pageId, {
    state_vector: toBase64(Y.encodeStateVector(doc)),
    update: toBase64(Y.encodeStateAsUpdate(doc, fromBase64(server))),
  });
}

async function openDoc(calls: SaveCalls, pageId: string) {
  const view = await calls.read(pageId);
  const doc = new Y.Doc();
  Y.applyUpdate(doc, fromBase64(view.document.update));
  return { doc, vector: view.document.state_vector };
}

/**
 * Save to a page: make files of the attachments (a file over the cap, or any
 * refusal, stops everything first), then add the blocks and the file blocks
 * in one sync. Nothing is half-added: if the sync fails, the files are let
 * go again. Throws {@link SaveRefused} for a refusal worth telling, else the
 * failure itself.
 */
export async function saveToPage(calls: SaveCalls, request: SaveRequest): Promise<Saved> {
  const asked = request.attachments.flatMap((group) => group.files);
  const over = tooLarge(asked);
  if (over !== undefined) {
    throw new SaveRefused({ reason: "tooLarge", name: over.name });
  }
  const attached: AttachedFile[] = [];
  for (const group of request.attachments) {
    try {
      attached.push(
        ...(await calls.attach(
          request.pageId,
          request.conversation,
          group.message,
          group.files.map((file) => file.id),
        )),
      );
    } catch (error) {
      await calls.detach(
        request.pageId,
        attached.map((file) => file.blob),
      );
      throw new SaveRefused(refusalOf(error, group.files));
    }
  }
  const blobs = [...new Set(attached.map((file) => file.blob))];
  try {
    const { doc, vector } = await openDoc(calls, request.pageId);
    const signatures = appendBlocks(doc, [...request.blocks, ...attached.map(fileBlock)]);
    await send(calls, request.pageId, doc, vector);
    return { pageId: request.pageId, signatures, blobs };
  } catch (error) {
    await calls.detach(request.pageId, blobs).catch(noop);
    throw error;
  }
}

/**
 * Take a save back: remove the blocks it added (the ones someone changed
 * since stay), then let go of the files it attached (the server keeps any a
 * block still names).
 */
export async function takeBack(calls: SaveCalls, saved: Saved): Promise<void> {
  const { doc, vector } = await openDoc(calls, saved.pageId);
  removeBlocks(doc, saved.signatures);
  await send(calls, saved.pageId, doc, vector);
  await calls.detach(saved.pageId, saved.blobs);
}
