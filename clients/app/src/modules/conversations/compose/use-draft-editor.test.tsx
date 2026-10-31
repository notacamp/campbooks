import { act } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { DUARTE } from "../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../test/render";
import { uploadAttachment, type Draft } from "../api/drafts";

import { useDraftEditor } from "./use-draft-editor";

const DRAFT_ID = "0192f1a0-0000-7000-8000-00000000d001";
const DRAFT_PATH = `${W}/drafts/${DRAFT_ID}`;

const draftAt = (version: number, attachments: Draft["attachments"] = []): Draft => ({
  id: DRAFT_ID,
  inbox_id: "0192f1a0-0000-7000-8000-00000000a001",
  kind: "new",
  from: { address: "sam@example.com" },
  to: [{ address: "leo@example.com" }],
  cc: [],
  bcc: [],
  subject: "Lease",
  body: "Attached.",
  quoted: "",
  attachments,
  version,
  state: "draft",
  created_at: "2026-10-28T09:00:00Z",
  updated_at: "2026-10-28T09:00:00Z",
  content_token: `token-${String(version)}`,
});

const uploaded = (n: number, name: string): Draft["attachments"][number] => ({
  id: `0192f1a0-0000-7000-8000-0000000000f${String(n)}`,
  source: "uploaded",
  filename: name,
  content_type: "application/pdf",
  size: 4,
});

interface Seen {
  method: string;
  path: string;
  search: URLSearchParams;
  headers: Headers;
  text: string;
}

/** Stubs fetch: `answer` decides each request (and sees the ones before). */
function stub(answer: (seen: Seen, calls: Seen[]) => { status?: number; body?: unknown }) {
  const calls: Seen[] = [];
  globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const request = input instanceof Request ? input : new Request(input, init);
    const url = new URL(request.url);
    const seen: Seen = {
      method: request.method,
      path: url.pathname,
      search: url.searchParams,
      headers: request.headers,
      text: await request.text(),
    };
    calls.push(seen);
    const { status = 200, body } = answer(seen, calls);
    return Response.json(body ?? null, {
      status,
      headers: {
        "content-type": status >= 400 ? "application/problem+json" : "application/json",
      },
    });
  };
  return calls;
}

const problem = (status: number, code: string) => ({
  status,
  body: { type: "about:blank", title: "Problem", status, detail: "Detail", code },
});

type Editor = ReturnType<typeof useDraftEditor>;

function mount(draft: Draft) {
  const box: { editor?: Editor } = {};
  function Probe() {
    box.editor = useDraftEditor(DUARTE.id, draft);
    return null;
  }
  renderWithProviders(<Probe />);
  return () => {
    if (box.editor === undefined) {
      throw new Error("the editor is not mounted");
    }
    return box.editor;
  };
}

const pdf = (name: string) => new File(["%PDF"], name, { type: "application/pdf" });

describe("uploadAttachment", () => {
  it("posts the file's bytes with the version and the token, and answers the draft", async () => {
    const calls = stub(() => ({ status: 201, body: draftAt(2, [uploaded(1, "lease.pdf")]) }));
    const answer = await uploadAttachment(
      DUARTE.id,
      { id: DRAFT_ID, version: 1, content_token: "token-1" },
      pdf("lease 4b.pdf"),
    );
    expect(answer.version).toBe(2);
    const [call] = calls;
    expect(call?.method).toBe("POST");
    expect(call?.path).toBe(`${DRAFT_PATH}/attachments`);
    expect(call?.search.get("version")).toBe("1");
    expect(call?.search.get("filename")).toBe("lease 4b.pdf");
    expect(call?.headers.get("x-content-token")).toBe("token-1");
    expect(call?.headers.get("content-type")).toBe("application/pdf");
    expect(call?.text).toBe("%PDF");
  });

  it("sends a file of no known type as an octet stream, and no token header without a token", async () => {
    const calls = stub(() => ({ status: 201, body: draftAt(2) }));
    await uploadAttachment(DUARTE.id, { id: DRAFT_ID, version: 1 }, new File(["x"], "notes"));
    expect(calls[0]?.headers.get("content-type")).toBe("application/octet-stream");
    expect(calls[0]?.headers.has("x-content-token")).toBe(false);
  });
});

describe("useDraftEditor.attach", () => {
  it("saves what was typed first, then uploads naming the save's version and token", async () => {
    const calls = stub((seen) =>
      seen.method === "PATCH"
        ? { body: draftAt(2) }
        : { status: 201, body: draftAt(3, [uploaded(1, "lease.pdf")]) },
    );
    const editor = mount(draftAt(1));
    act(() => {
      editor().update({ body: "Attached, thanks." });
    });
    let refused: unknown;
    await act(async () => {
      refused = await editor().attach([pdf("lease.pdf")]);
    });
    expect(refused).toEqual([]);
    expect(calls.map((c) => c.method)).toEqual(["PATCH", "POST"]);
    expect(JSON.parse(calls[0]?.text ?? "{}")).toMatchObject({
      version: 1,
      content_token: "token-1",
      body: "Attached, thanks.",
    });
    expect(calls[1]?.search.get("version")).toBe("2");
    expect(calls[1]?.headers.get("x-content-token")).toBe("token-2");
    expect(editor().shown.attachments.map((a) => a.filename)).toEqual(["lease.pdf"]);
    expect(editor().fields.attachments).toEqual([uploaded(1, "lease.pdf").id]);
    expect(editor().saveState).toBe("saved");
    expect(editor().uploading).toEqual([]);
  });

  it("keeps the uploaded file in the next save, and what was typed during the upload", async () => {
    const calls = stub((seen) =>
      seen.method === "POST"
        ? { status: 201, body: draftAt(2, [uploaded(1, "lease.pdf")]) }
        : { body: draftAt(3, [uploaded(1, "lease.pdf")]) },
    );
    // The upload is held open while the member types.
    const holder: { release?: (go: string) => void } = {};
    const gate = new Promise<string>((resolve) => {
      holder.release = resolve;
    });
    const answering = globalThis.fetch;
    globalThis.fetch = async (input, init) => {
      const request = input instanceof Request ? input.clone() : new Request(input, init);
      if (request.method === "POST") {
        await gate;
      }
      return answering(input, init);
    };
    const editor = mount(draftAt(1));
    let done: Promise<unknown> = Promise.resolve();
    await act(async () => {
      done = editor().attach([pdf("lease.pdf")]);
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    act(() => {
      editor().update({ subject: "Lease 4B" });
    });
    await act(async () => {
      holder.release?.("go");
      await done;
    });
    expect(editor().fields.subject).toBe("Lease 4B");
    expect(editor().saveState).toBe("unsaved");
    await act(async () => {
      await editor().flush();
    });
    const save = calls.findLast((c) => c.method === "PATCH");
    expect(JSON.parse(save?.text ?? "{}")).toMatchObject({
      version: 2,
      content_token: "token-2",
      subject: "Lease 4B",
      attachments: [uploaded(1, "lease.pdf").id],
    });
  });

  it("uploads files one after the other, each naming the last answer", async () => {
    const calls = stub((_seen, all) => {
      const posts = all.filter((c) => c.method === "POST").length;
      return {
        status: 201,
        body: draftAt(
          1 + posts,
          Array.from({ length: posts }, (_, i) => uploaded(i + 1, `f${String(i + 1)}.pdf`)),
        ),
      };
    });
    const editor = mount(draftAt(1));
    await act(async () => {
      await editor().attach([pdf("f1.pdf"), pdf("f2.pdf")]);
    });
    expect(calls.map((c) => c.search.get("version"))).toEqual(["1", "2"]);
    expect(calls.map((c) => c.headers.get("x-content-token"))).toEqual(["token-1", "token-2"]);
    expect(editor().fields.attachments).toHaveLength(2);
  });

  it("reports a refused file and goes on with the others", async () => {
    stub((seen) =>
      seen.search.get("filename") === "huge.bin"
        ? problem(413, "attachments_too_large")
        : { status: 201, body: draftAt(2, [uploaded(1, "ok.pdf")]) },
    );
    const editor = mount(draftAt(1));
    let refused: { file: File; code: string }[] = [];
    await act(async () => {
      refused = await editor().attach([new File(["x"], "huge.bin"), pdf("ok.pdf")]);
    });
    expect(refused.map((r) => [r.file.name, r.code])).toEqual([
      ["huge.bin", "attachments_too_large"],
    ]);
    expect(editor().fields.attachments).toEqual([uploaded(1, "ok.pdf").id]);
    expect(editor().error).toBeUndefined();
  });

  it("handles a draft changed elsewhere as a save does: reads it again and says so", async () => {
    const calls = stub((seen) => {
      if (seen.method === "POST") {
        return problem(409, "draft_changed");
      }
      return { body: draftAt(5) };
    });
    const editor = mount(draftAt(1));
    let refused: { code: string }[] = [];
    await act(async () => {
      refused = await editor().attach([pdf("lease.pdf")]);
    });
    expect(refused.map((r) => r.code)).toEqual(["draft_changed"]);
    expect(calls.map((c) => c.method)).toEqual(["POST", "GET"]);
    expect(editor().error).toBe("content_changed");
    expect(editor().fields.attachments).toEqual([]);
    // The next upload names what was read.
    stub(() => ({ status: 201, body: draftAt(6, [uploaded(1, "lease.pdf")]) }));
    const next = stubCalls();
    await act(async () => {
      await editor().attach([pdf("lease.pdf")]);
    });
    expect(next()[0]?.search.get("version")).toBe("5");
    expect(next()[0]?.headers.get("x-content-token")).toBe("token-5");
  });
});

/** The calls of the stub installed last (a second `stub` replaces the first). */
function stubCalls() {
  const original = globalThis.fetch;
  const calls: Seen[] = [];
  globalThis.fetch = async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init);
    const url = new URL(request.url);
    calls.push({
      method: request.method,
      path: url.pathname,
      search: url.searchParams,
      headers: request.headers,
      text: "",
    });
    return original(input, init);
  };
  return () => calls;
}

describe("useDraftEditor.send", () => {
  it("sends now without a send_at, and at a chosen time with one", async () => {
    const calls = stub(() => ({ status: 202, body: draftAt(2) }));
    const editor = mount(draftAt(1));
    await act(async () => {
      await editor().send();
    });
    const at = new Date(Date.UTC(2026, 9, 29, 8, 0));
    await act(async () => {
      await editor().send(at);
    });
    const bodies = calls
      .filter((c) => c.method === "POST")
      .map((c) => JSON.parse(c.text) as Record<string, unknown>);
    expect(bodies[0]).toEqual({ version: 1, content_token: "token-1" });
    expect(bodies[1]).toMatchObject({ send_at: "2026-10-29T08:00:00.000Z" });
  });

  it("says why when the server will not take the time", async () => {
    stub(() => problem(400, "send_at_invalid"));
    const editor = mount(draftAt(1));
    let sent: unknown = "unset";
    await act(async () => {
      sent = await editor().send(new Date(Date.UTC(2020, 0, 1)));
    });
    expect(sent).toBeUndefined();
    expect(editor().error).toBe("send_at_invalid");
  });
});
