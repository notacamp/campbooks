// Fictional chats with Scout for tests and stories (never a real person):
// Sam Rivera asking about Marta Ferreira's walkthrough email, as the
// assistant API returns them (campbooks-cloud#305).
import type { Schema } from "~/lib/api";

import { MARTA, PEDRO } from "./attention";
import { BILL, LEASE, PROPOSAL } from "./tasks";
import { RUI } from "./workspaces";

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

// Scout proposes task changes (campbooks-cloud#43): Sam says what happened
// to three of the list's tasks; Scout proposes each, as cards that name the
// task from its record.

export const TASKS_SAID =
  "I paid the electricity bill, I will send Marta the lease on Friday, and Rui can take the proposal draft.";

/** The request a task card sends: the revision read when it was proposed. */
const taskRequest = (operation: string, tail: string, task: { id: string }, body: unknown) => ({
  operation,
  method: "POST" as const,
  path: `tasks/${task.id}${tail}`,
  body,
});

const taskUndo = (operation: string, tail: string, task: { id: string }, body: unknown = null) => ({
  ...taskRequest(operation, tail, task, body),
  from_response: ["revision"],
});

const taskFacts = (task: { title: string; verb: string; due_on?: string | null }) => ({
  title: task.title,
  verb: task.verb,
  ...(task.due_on === null || task.due_on === undefined ? {} : { due_on: task.due_on }),
});

export const BILL_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000003061",
  kind: "change",
  state: "pending",
  action: "task_done",
  record: { type: "tasks.task", id: BILL.id },
  task: taskFacts(BILL),
  detail: {},
  request: taskRequest("tasks.done", "/done", BILL, { revision: BILL.revision }),
  undo: taskUndo("tasks.reopen", "/reopen", BILL),
};

export const LEASE_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000003062",
  kind: "change",
  state: "pending",
  action: "plan_day",
  record: { type: "tasks.task", id: LEASE.id },
  task: taskFacts(LEASE),
  detail: { day: "2026-10-16" },
  request: taskRequest("tasks.plan", "/plan", LEASE, {
    revision: LEASE.revision,
    plan: "date",
    plan_on: "2026-10-16",
  }),
  undo: taskUndo("tasks.plan", "/plan", LEASE, { plan: "none" }),
};

export const HAND_OFF_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000003063",
  kind: "change",
  state: "pending",
  action: "hand_off",
  record: { type: "tasks.task", id: PROPOSAL.id },
  task: taskFacts(PROPOSAL),
  detail: { member_id: RUI.user_id, member_name: RUI.display_name },
  request: taskRequest("tasks.hand_off", "/hand_off", PROPOSAL, {
    revision: PROPOSAL.revision,
    to: RUI.user_id,
  }),
  undo: taskUndo("tasks.take_back", "/take_back", PROPOSAL),
};

/** Scout's answer to {@link TASKS_SAID}, with its three task cards. */
export const TASKS_ANSWER: Message = {
  id: "0192f1a0-0000-7000-8000-000000003052",
  author: "scout",
  text:
    "I proposed marking the electricity bill paid, planning the lease for Friday and " +
    "handing the proposal draft to Rui. Nothing changes until you confirm each card.",
  steps: [],
  finish: "done",
  proposals: [BILL_PROPOSAL, LEASE_PROPOSAL, HAND_OFF_PROPOSAL],
  created_at: "2026-10-13T14:59:40Z",
};

/** The chat once {@link TASKS_SAID} was answered, its cards in `proposals`' states. */
export function chatWithTaskProposals(
  proposals: readonly Proposal[] = TASKS_ANSWER.proposals,
): Chat {
  return {
    ...MARTA_CHAT,
    questions: 1,
    last_message_at: TASKS_ANSWER.created_at,
    messages: [
      {
        id: "0192f1a0-0000-7000-8000-000000003051",
        author: "member",
        text: TASKS_SAID,
        steps: [],
        finish: null,
        proposals: [],
        created_at: "2026-10-13T14:59:30Z",
      },
      { ...TASKS_ANSWER, proposals: [...proposals] },
    ],
  };
}
