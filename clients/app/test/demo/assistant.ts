// Fictional chats with Scout for tests and stories (never a real person):
// Sam Rivera asking about Marta Ferreira's walkthrough email, as the
// assistant API returns them (campbooks-cloud#305).
import type { Schema } from "~/lib/api";

import { MARTA, PEDRO } from "./attention";

type Chat = Schema<"ChatView">;
type Message = Schema<"ChatMessageView">;

const CHAT_ID = "0192f1a0-0000-7000-8000-000000003001";

/** Sam's new chat about Marta's email: nothing asked yet. */
export const MARTA_CHAT: Chat = {
  id: CHAT_ID,
  about: { type: "conversations.conversation", id: MARTA.id },
  questions: 0,
  max_questions: 40,
  created_at: "2026-10-13T14:58:00Z",
  last_message_at: "2026-10-13T14:58:00Z",
  messages: [],
};

export const WHY = "Why is this here?";

/** Scout's answer to {@link WHY}, as stored. */
export const WHY_ANSWER: Message = {
  id: "0192f1a0-0000-7000-8000-000000003012",
  author: "scout",
  text:
    "It is under Reply because Marta asks you to confirm Thursday’s walkthrough, " +
    "and the builders need to know by Wednesday.\n\n" +
    "- Scout read that it needs a reply, 90% sure.\n" +
    "- You wrote to Marta about the lease two weeks ago.",
  steps: [
    {
      tool: "conversation_history",
      records: [{ type: "conversations.conversation", id: MARTA.id }],
      count: 1,
    },
    {
      tool: "find_conversations",
      records: [{ type: "conversations.conversation", id: PEDRO.id }],
      count: 1,
    },
  ],
  finish: "done",
  created_at: "2026-10-13T14:59:10Z",
};

const QUESTION: Message = {
  id: "0192f1a0-0000-7000-8000-000000003011",
  author: "member",
  text: WHY,
  steps: [],
  finish: null,
  created_at: "2026-10-13T14:59:00Z",
};

/** The same chat once {@link WHY} was answered. */
export const MARTA_CHAT_ANSWERED: Chat = {
  ...MARTA_CHAT,
  questions: 1,
  last_message_at: WHY_ANSWER.created_at,
  messages: [QUESTION, WHY_ANSWER],
};

/** The chat with {@link WHY} asked and Scout's answer failed. */
export const MARTA_CHAT_FAILED: Chat = {
  ...MARTA_CHAT,
  questions: 1,
  messages: [QUESTION, { ...WHY_ANSWER, text: "", steps: [], finish: "failed" }],
};

/** The frames of a streamed answer: thinking, a step per tool, the words in pieces, then done. */
export function answerFrames(answer: Message, thinking = "Marta asks Sam to confirm."): string[] {
  const frame = (event: string, data: unknown) =>
    `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
  const words = answer.text.split(/(?<= )/u);
  return [
    frame("thinking", { delta: thinking }),
    ...answer.steps.map((step) => frame("step", { tool: step.tool })),
    ...words.map((delta) => frame("text", { delta })),
    frame("done", answer),
  ];
}
