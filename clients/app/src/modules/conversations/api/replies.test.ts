import { describe, expect, it, vi } from "vitest";

import { problemCode } from "~/lib/api";

import { Q4, W } from "../../../../test/demo/conversations";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, problem } from "../../../../test/render";

import { streamReply } from "./replies";

const STREAM = `POST ${W}/conversations/${Q4.id}/reply/draft/stream`;
const frame = (event: string, data: unknown) =>
  `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;

function ask(onThinking = vi.fn(), onText = vi.fn()) {
  return streamReply(
    DUARTE.id,
    Q4.id,
    { intent: "Say yes", suggestions: ["Say yes", "Say no"] },
    { signal: new AbortController().signal, onThinking, onText },
  );
}

describe("streamReply", () => {
  it("hands on the thinking and the words, and answers the whole reply", async () => {
    const api = mockApi({
      [STREAM]: {
        stream: [
          frame("thinking", { delta: "Maya asks." }) + ": keep-alive\n\n",
          frame("text", { delta: "Hi " }),
          frame("text", { delta: "Maya" }),
          frame("done", { message_id: "m1", text: "Hi Maya" }),
        ],
      },
    });
    const onThinking = vi.fn();
    const onText = vi.fn();

    await expect(ask(onThinking, onText)).resolves.toBe("Hi Maya");
    expect(onThinking).toHaveBeenCalledWith("Maya asks.");
    expect(onText.mock.calls).toEqual([["Hi "], ["Maya"]]);
    expect(api.calls[0]?.body).toEqual({ intent: "Say yes", suggestions: ["Say yes", "Say no"] });
  });

  it("throws the problem an error frame carries", async () => {
    mockApi({
      [STREAM]: {
        stream: [frame("error", { status: 503, code: "reply_timed_out", detail: "Too long" })],
      },
    });

    const thrown = await ask().catch((error: unknown) => error);
    expect(problemCode(thrown)).toBe("reply_timed_out");
  });

  it("throws the refusal before any stream", async () => {
    mockApi({ [STREAM]: problem(409, "reply_unavailable") });

    const thrown = await ask().catch((error: unknown) => error);
    expect(problemCode(thrown)).toBe("reply_unavailable");
  });

  it("fails a stream that ends without the reply", async () => {
    mockApi({ [STREAM]: { stream: [frame("text", { delta: "Hi" })] } });

    const thrown = await ask().catch((error: unknown) => error);
    expect(problemCode(thrown)).toBe("reply_failed");
  });

  it("ignores frames it does not know", async () => {
    mockApi({
      [STREAM]: {
        stream: [frame("later", {}) + frame("done", { message_id: "m1", text: "Hi" })],
      },
    });

    await expect(ask()).resolves.toBe("Hi");
  });
});
