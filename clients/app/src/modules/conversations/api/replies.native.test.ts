import { describe, expect, it, vi } from "vitest";

import { Q4, W } from "../../../../test/demo/conversations";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi } from "../../../../test/render";

import { streamReply } from "./replies";

// The desktop app's bridge hands back whole answers: no streaming there.
vi.mock("~/lib/api", async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  canStream: false,
}));

describe("streamReply where answers cannot stream", () => {
  it("asks for the reply as JSON and shows it whole", async () => {
    const api = mockApi({
      [`POST ${W}/conversations/${Q4.id}/reply/draft`]: {
        body: { message_id: "m1", text: "Hi Maya" },
      },
    });
    const onText = vi.fn();

    await expect(
      streamReply(
        DUARTE.id,
        Q4.id,
        { intent: "Say yes" },
        { signal: new AbortController().signal, onThinking: vi.fn(), onText },
      ),
    ).resolves.toBe("Hi Maya");
    expect(onText).not.toHaveBeenCalled();
    expect(api.calls.map((call) => call.route)).toEqual([
      `POST ${W}/conversations/${Q4.id}/reply/draft`,
    ]);
  });
});
