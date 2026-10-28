import { describe, expect, it } from "vitest";
import * as Y from "yjs";

import { ApiError } from "~/lib/api";

import { fromBase64, toBase64 } from "../editor/bytes";
import type { AttachedFile, PageView } from "../types";

import { appendBlocks, removeBlocks, threadBlock } from "./blocks";
import { buildCopy } from "./copy";
import { FILE_MAX_BYTES } from "./refusals";
import { saveToPage, SaveRefused, takeBack, type SaveCalls } from "./save";

const THREAD = "0192f1a0-0000-7000-8000-00000000a001";
const MESSAGE = "0192f1a0-0000-7000-8000-00000000b001";
const PAGE = "0192f1a0-0000-7000-8000-00000000c001";

const paragraph = (text: string) => ({ type: "paragraph", content: [{ type: "text", text }] });

/** The pages API as a server holds it: one page's document, merged on sync, and its files. */
function fakeServer(options: { attach?: () => Promise<AttachedFile[]>; failSync?: boolean } = {}) {
  const doc = new Y.Doc();
  appendBlocks(doc, [paragraph("Before the kickoff"), paragraph("Send the welcome pack")]);
  const held = new Set<string>();
  const log: string[] = [];
  const calls: SaveCalls = {
    read: (pageId) => {
      log.push(`read ${pageId}`);
      return Promise.resolve({
        document: {
          update: toBase64(Y.encodeStateAsUpdate(doc)),
          state_vector: toBase64(Y.encodeStateVector(doc)),
        },
      } as unknown as PageView);
    },
    sync: (_pageId, body) => {
      log.push("sync");
      if (options.failSync === true) {
        return Promise.reject(new ApiError(500, undefined));
      }
      if (body.update !== undefined) {
        Y.applyUpdate(doc, fromBase64(body.update));
      }
      return Promise.resolve({
        revision: 2,
        state_vector: toBase64(Y.encodeStateVector(doc)),
        update: toBase64(Y.encodeStateAsUpdate(doc, fromBase64(body.state_vector))),
      });
    },
    attach: (_page, _conversation, _message, attachments) => {
      log.push(`attach ${attachments.join(",")}`);
      if (options.attach !== undefined) {
        return options.attach();
      }
      const files = attachments.map((id) => ({
        attachment: id,
        blob: `hash-${id}`,
        content_type: id === "a-logo" ? "image/png" : "application/pdf",
        name: id === "a-logo" ? "logo.png" : "brand-guide.pdf",
        size: 2048,
      }));
      for (const file of files) {
        held.add(file.blob);
      }
      return Promise.resolve(files);
    },
    detach: (_page, blobs) => {
      log.push(`detach ${blobs.join(",")}`);
      for (const blob of blobs) {
        held.delete(blob);
      }
      return Promise.resolve();
    },
  };
  const body = () => doc.getXmlFragment("default").length;
  const types = () =>
    doc
      .getXmlFragment("default")
      .slice(0)
      .map((node) => (node instanceof Y.XmlElement ? node.nodeName : "text"));
  return { doc, calls, held, log, body, types };
}

describe("saveToPage", () => {
  it("adds a live thread block at the end of the page", async () => {
    const server = fakeServer();
    const saved = await saveToPage(server.calls, {
      pageId: PAGE,
      conversation: THREAD,
      blocks: [threadBlock(THREAD)],
      attachments: [],
    });
    expect(server.types()).toEqual(["paragraph", "paragraph", "embed"]);
    expect(saved.signatures).toHaveLength(1);
    expect(server.doc.getXmlFragment("default").get(2)).toBeInstanceOf(Y.XmlElement);
    const embed = server.doc.getXmlFragment("default").get(2) as Y.XmlElement;
    expect(embed.getAttribute("ref_type")).toBe("conversations.conversation");
    expect(embed.getAttribute("ref_id")).toBe(THREAD);
  });

  it("adds a copy: a heading per message, then its words", async () => {
    const server = fakeServer();
    await saveToPage(server.calls, {
      pageId: PAGE,
      conversation: THREAD,
      blocks: buildCopy(
        [{ from: "Ana", sentAt: "2026-10-21T09:00:00Z", text: "Three things for Thursday." }],
        (iso) => iso.slice(0, 10),
      ),
      attachments: [],
    });
    expect(server.types()).toEqual(["paragraph", "paragraph", "heading", "paragraph"]);
  });

  it("attaches the files and adds their blocks in the same sync", async () => {
    const server = fakeServer();
    const saved = await saveToPage(server.calls, {
      pageId: PAGE,
      conversation: THREAD,
      blocks: [threadBlock(THREAD)],
      attachments: [
        {
          message: MESSAGE,
          files: [
            { id: "a-guide", name: "brand-guide.pdf", size: 2048 },
            { id: "a-logo", name: "logo.png", size: 2048 },
          ],
        },
      ],
    });
    expect(server.types()).toEqual(["paragraph", "paragraph", "embed", "file", "image"]);
    expect(server.log.filter((entry) => entry === "sync")).toHaveLength(1);
    expect(saved.blobs).toEqual(["hash-a-guide", "hash-a-logo"]);
  });

  it("refuses a file over 20 MiB by name before anything is asked of the server", async () => {
    const server = fakeServer();
    await expect(
      saveToPage(server.calls, {
        pageId: PAGE,
        conversation: THREAD,
        blocks: [threadBlock(THREAD)],
        attachments: [
          {
            message: MESSAGE,
            files: [{ id: "a-big", name: "logo-pack.zip", size: FILE_MAX_BYTES + 1 }],
          },
        ],
      }),
    ).rejects.toMatchObject({ refusal: { reason: "tooLarge", name: "logo-pack.zip" } });
    expect(server.log).toEqual([]);
    expect(server.body()).toBe(2);
  });

  it("says why the server refused the attachments, and lets go of the ones already made", async () => {
    const server = fakeServer();
    let calls = 0;
    const flaky: SaveCalls = {
      ...server.calls,
      attach: (...args) => {
        calls += 1;
        return calls === 1
          ? server.calls.attach(...args)
          : Promise.reject(new ApiError(404, undefined));
      },
    };
    const error = await saveToPage(flaky, {
      pageId: PAGE,
      conversation: THREAD,
      blocks: [threadBlock(THREAD)],
      attachments: [
        { message: MESSAGE, files: [{ id: "a-guide", name: "brand-guide.pdf", size: 10 }] },
        {
          message: "0192f1a0-0000-7000-8000-00000000b002",
          files: [{ id: "a-logo", name: "logo.png", size: 10 }],
        },
      ],
    }).catch((caught: unknown) => caught);
    expect(error).toBeInstanceOf(SaveRefused);
    expect((error as SaveRefused).refusal).toEqual({ reason: "changed" });
    // Nothing is half-added: no blocks, and the first message's file is let go.
    expect(server.body()).toBe(2);
    expect(server.held.size).toBe(0);
  });

  it("lets go of the files if the page could not be written", async () => {
    const server = fakeServer({ failSync: true });
    await expect(
      saveToPage(server.calls, {
        pageId: PAGE,
        conversation: THREAD,
        blocks: [threadBlock(THREAD)],
        attachments: [
          { message: MESSAGE, files: [{ id: "a-guide", name: "brand-guide.pdf", size: 10 }] },
        ],
      }),
    ).rejects.toBeInstanceOf(ApiError);
    expect(server.held.size).toBe(0);
    expect(server.body()).toBe(2);
  });
});

describe("takeBack (the Undo)", () => {
  it("removes the blocks that were added and detaches the files", async () => {
    const server = fakeServer();
    const saved = await saveToPage(server.calls, {
      pageId: PAGE,
      conversation: THREAD,
      blocks: [threadBlock(THREAD)],
      attachments: [
        { message: MESSAGE, files: [{ id: "a-guide", name: "brand-guide.pdf", size: 10 }] },
      ],
    });
    expect(server.types()).toEqual(["paragraph", "paragraph", "embed", "file"]);
    await takeBack(server.calls, saved);
    expect(server.types()).toEqual(["paragraph", "paragraph"]);
    expect(server.held.size).toBe(0);
  });

  it("keeps a block someone changed since, and what was written around it", async () => {
    const server = fakeServer();
    const saved = await saveToPage(server.calls, {
      pageId: PAGE,
      conversation: THREAD,
      blocks: buildCopy(
        [{ from: "Ana", sentAt: "2026-10-21T09:00:00Z", text: "Three things for Thursday." }],
        (iso) => iso.slice(0, 10),
      ),
      attachments: [],
    });
    // Rui edits the copied paragraph, and writes a new one after it.
    const body = server.doc.getXmlFragment("default");
    const copied = body.get(3) as Y.XmlElement;
    (copied.get(0) as Y.XmlText).insert(0, "Rui: ");
    appendBlocks(server.doc, [paragraph("Rui's own note")]);
    await takeBack(server.calls, saved);
    expect(server.types()).toEqual(["paragraph", "paragraph", "paragraph", "paragraph"]);
    const text = body
      .slice(0)
      .map((node) => (node instanceof Y.XmlElement ? node.toJSON() : ""))
      .join("");
    expect(text).toContain("Rui: Three things for Thursday.");
    expect(text).toContain("Rui&apos;s own note".replace("&apos;", "'"));
    expect(text).not.toContain("Ana");
  });
});

describe("removeBlocks", () => {
  it("removes only the last block that still reads the same", () => {
    const doc = new Y.Doc();
    appendBlocks(doc, [threadBlock(THREAD)]);
    const signatures = appendBlocks(doc, [threadBlock(THREAD)]);
    expect(removeBlocks(doc, signatures)).toBe(1);
    expect(doc.getXmlFragment("default").length).toBe(1);
  });
});
