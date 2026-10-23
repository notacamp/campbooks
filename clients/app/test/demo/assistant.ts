// Fictional chats with Scout for tests and stories (never a real person):
// Sam Rivera asking about Marta Ferreira's walkthrough email, as the
// assistant API returns them (campbooks-cloud#305).
import type { Schema } from "~/lib/api";

import { MARTA, PEDRO } from "./attention";
import { BUILDS_NOTICE, LEDGER_RULE } from "./conversations";

type Chat = Schema<"ChatView">;
type Message = Schema<"ChatMessageView">;
type Proposal = Schema<"ChatProposal">;

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
  proposals: [],
  created_at: "2026-10-13T14:59:10Z",
};

const QUESTION: Message = {
  id: "0192f1a0-0000-7000-8000-000000003011",
  author: "member",
  text: WHY,
  steps: [],
  finish: null,
  proposals: [],
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

// Scout proposes (campbooks-cloud#305, phase 2): Sam asks Scout to clear
// Marta's email; Scout proposes archiving it and a rule, as cards.

export const CLEAR = "I answered Marta by phone. Can you get this out of my inbox?";

/** Scout's proposal to archive Marta's email, as the server built it. */
export const ARCHIVE_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000003031",
  kind: "change",
  state: "pending",
  action: "archive",
  record: { type: "conversations.conversation", id: MARTA.id },
  facts: { sender: "Marta Ferreira", subject: MARTA.subject, stream: "primary" },
  detail: {},
  request: {
    operation: "conversations.archive",
    method: "POST",
    path: `conversations/${MARTA.id}/archive`,
    body: null,
  },
  undo: {
    operation: "conversations.unarchive",
    method: "POST",
    path: `conversations/${MARTA.id}/unarchive`,
    body: null,
  },
};

export const RULE_SENTENCE = "Mail from Marta Ferreira always needs a reply";

/** Scout's proposal of a rule, for Teach Scout. */
export const RULE_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000003032",
  kind: "rule",
  state: "pending",
  sentence: RULE_SENTENCE,
};

/** Scout's proposal of a reply to Marta. */
export const REPLY_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000003033",
  kind: "reply",
  state: "pending",
  record: { type: "conversations.conversation", id: MARTA.id },
  facts: { sender: "Marta Ferreira", subject: MARTA.subject, stream: "primary" },
  intent: "Confirm Thursday at 10 and ask who else is coming",
};

/** Scout's answer to {@link CLEAR}, with its cards. */
export const CLEAR_ANSWER: Message = {
  id: "0192f1a0-0000-7000-8000-000000003022",
  author: "scout",
  text:
    "I proposed archiving Marta’s email: confirm on the card and it leaves your inbox. " +
    "Her emails usually need an answer, so I also proposed a rule that marks her mail as needing one.",
  steps: [],
  finish: "done",
  proposals: [ARCHIVE_PROPOSAL, RULE_PROPOSAL],
  created_at: "2026-10-13T14:59:40Z",
};

const CLEAR_QUESTION: Message = {
  id: "0192f1a0-0000-7000-8000-000000003021",
  author: "member",
  text: CLEAR,
  steps: [],
  finish: null,
  proposals: [],
  created_at: "2026-10-13T14:59:30Z",
};

/** The chat once {@link CLEAR} was answered, its cards in `proposals`' states. */
export function chatWithProposals(proposals: readonly Proposal[] = CLEAR_ANSWER.proposals): Chat {
  return {
    ...MARTA_CHAT,
    questions: 1,
    last_message_at: CLEAR_ANSWER.created_at,
    messages: [CLEAR_QUESTION, { ...CLEAR_ANSWER, proposals: [...proposals] }],
  };
}

/** What Teach Scout reads in {@link RULE_SENTENCE}: one rule, in Sam's inbox. */
export const MARTA_RULE = {
  outcome: "rules",
  unsupported: null,
  text: RULE_SENTENCE,
  proposals: [
    {
      conditions: [{ field: "sender_address", value: "marta@casa-lume.example" }],
      actions: [{ action: "needs_reply", value: true }],
      inboxes: [
        {
          id: "0192f1a0-0000-7000-8000-0000000000b1",
          address: "sam.rivera@example.com",
          shared: false,
        },
      ],
      who: null,
    },
  ],
};

// Scout's chat from anywhere (campbooks-cloud#305, phase 3): about one of
// Scout's suggestions, about a rule, about nothing (⌘K), and Recent.

export const NOTICE_WHY = "Why do you suggest this?";
export const WAITING_QUESTION = "What am I waiting on?";

/** Sam's new chat about the build robot's suggestion. */
export const NOTICE_CHAT: Chat = {
  ...MARTA_CHAT,
  id: "0192f1a0-0000-7000-8000-000000003201",
  about: { type: "conversations.rule_notice", id: BUILDS_NOTICE.id },
};

/** Scout's answer to {@link NOTICE_WHY}, as stored. */
export const NOTICE_ANSWER: Message = {
  id: "0192f1a0-0000-7000-8000-000000003212",
  author: "scout",
  text:
    "CI sent you 34 build reports in the last 60 days and you opened 2 of them.\n\n" +
    "- They come most days, so they keep landing on Home.\n" +
    "- Filed under Updates, they would stay out of your way and still be there.",
  steps: [
    {
      tool: "find_conversations",
      records: [],
      count: 34,
    },
  ],
  finish: "done",
  proposals: [],
  created_at: "2026-10-13T15:01:10Z",
};

/** Sam's new chat about the Ledger rule. */
export const RULE_CHAT: Chat = {
  ...MARTA_CHAT,
  id: "0192f1a0-0000-7000-8000-000000003301",
  about: { type: "conversations.rule", id: LEDGER_RULE.id },
};

/** Sam's new chat about nothing in particular, asked from ⌘K. */
export const NOTHING_CHAT: Chat = {
  ...MARTA_CHAT,
  id: "0192f1a0-0000-7000-8000-000000003401",
  about: null,
};

/** Scout's answer to {@link WAITING_QUESTION}, as stored. */
export const WAITING_ANSWER: Message = {
  id: "0192f1a0-0000-7000-8000-000000003412",
  author: "scout",
  text:
    "You’re waiting on two people:\n\n" +
    "- Rui Matos, for the signed contract for the kitchen units, which he said he’d send by Friday.\n" +
    "- Inês Cardoso, for the revised quote for the bathroom and the hall. You nudged her once.",
  steps: [{ tool: "what_needs_me", records: [], count: 2 }],
  finish: "done",
  proposals: [],
  created_at: "2026-10-13T15:02:10Z",
};

/** A chat in {@link NOTHING_CHAT}'s shape once `answer` answered `question`. */
export function answered(chat: Chat, question: string, answer: Message): Chat {
  return {
    ...chat,
    questions: 1,
    last_message_at: answer.created_at,
    messages: [
      {
        id: `${answer.id.slice(0, -1)}1`,
        author: "member",
        text: question,
        steps: [],
        finish: null,
        proposals: [],
        created_at: answer.created_at,
      },
      answer,
    ],
  };
}

/** Sam's chats as Recent lists them, newest first. */
export const RECENT: Schema<"ChatSummary">[] = [
  {
    id: NOTHING_CHAT.id,
    about: null,
    first_question: WAITING_QUESTION,
    questions: 1,
    last_message_at: WAITING_ANSWER.created_at,
  },
  {
    id: MARTA_CHAT.id,
    about: MARTA_CHAT.about ?? null,
    first_question: WHY,
    questions: 1,
    last_message_at: "2026-10-12T09:40:00Z",
  },
  {
    id: RULE_CHAT.id,
    about: RULE_CHAT.about ?? null,
    first_question: "What has it changed?",
    questions: 2,
    last_message_at: "2026-10-09T17:05:00Z",
  },
];
