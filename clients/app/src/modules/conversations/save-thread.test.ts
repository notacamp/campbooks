import { describe, expect, it } from "vitest";

import { Q4_DETAIL } from "../../../test/demo/conversations";

import type { MessageView } from "./api/queries";
import { saveThreadOf } from "./save-thread";

const [base] = Q4_DETAIL.messages;
if (base === undefined) {
  throw new Error("the demo thread has no messages");
}
const message = (over: Partial<MessageView>): MessageView => ({ ...base, ...over });

describe("saveThreadOf", () => {
  const thread = saveThreadOf("c-1", "Kickoff agenda", [
    message({
      id: "m-2",
      sent_at: "2026-10-22T10:00:00Z",
      from: { name: "Ana Duarte", address: "ana@example.com" },
      text: { content: "Thanks Jonas.\n\n> Hi Ana,\n> three things.", truncated: false },
      attachments: [],
    }),
    message({
      id: "m-1",
      sent_at: "2026-10-21T09:00:00Z",
      from: { name: null, address: "jonas@example.com" },
      text: { content: "Hi Ana, three things.", truncated: false },
      attachments: [
        {
          id: "a-1",
          filename: "brand-guide.pdf",
          content_type: "application/pdf",
          size: 2048,
          in_body: false,
          inline: false,
        },
        {
          id: "a-2",
          filename: "signature.png",
          content_type: "image/png",
          size: 12,
          in_body: true,
          inline: true,
        },
      ],
    }),
  ]);

  it("is oldest first, by the name the reader shows", () => {
    expect(thread.messages.map((m) => [m.id, m.from])).toEqual([
      ["m-1", "jonas@example.com"],
      ["m-2", "Ana Duarte"],
    ]);
  });

  it("drops the quoted history, as the reader folds it away", () => {
    expect(thread.messages[1]?.text).toBe("Thanks Jonas.");
    expect(thread.messages[0]?.text).toBe("Hi Ana, three things.");
  });

  it("lists the files attached, not the pictures the body shows", () => {
    expect(thread.messages[0]?.attachments).toEqual([
      { id: "a-1", name: "brand-guide.pdf", size: 2048 },
    ]);
  });
});
