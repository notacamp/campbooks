// Fictional chats with Scout for tests and stories (never a real person):
// Sam Rivera asking about Marta Ferreira's walkthrough email, as the
// assistant API returns them (campbooks-cloud#305).
import type { Schema } from "~/lib/api";

import { MARTA, PEDRO } from "./attention";
import { CALENDARS, DENTIST, HANDOVER, REVIEW } from "./schedule";
import { BILL, LEASE, PROPOSAL } from "./tasks";
import { RUI } from "./workspaces";
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

/** Sam's new chat about the lease task. */
export const TASK_CHAT: Chat = {
  ...MARTA_CHAT,
  id: "0192f1a0-0000-7000-8000-000000003501",
  about: { type: "tasks.task", id: LEASE.id },
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

// Scout proposes calendar changes (campbooks-cloud#42): Sam asks to move the
// dentist, answer an invitation and put the lease handover on the Google
// calendar; Scout proposes each as a card that names the event from its
// record, with the request and the undo the server writes for it.

export const CALENDAR_SAID =
  "Move the dentist to Thursday at 10:00, accept the design review and put the key handover on my Google calendar.";

const MOVED_START = "2026-10-29T10:00:00+00:00";
const MOVED_END = "2026-10-29T10:45:00+00:00";
const PERSONAL = CALENDARS.find((calendar) => calendar.name === "Personal");
const WORK = CALENDARS.find((calendar) => calendar.name === "Work");

const eventPath = (event: { id: string }, tail = "") => `schedule/events/${event.id}${tail}`;

const eventFacts = (
  event: { title: string; starts: string; all_day: boolean },
  calendar: string | undefined,
) => ({
  title: event.title,
  starts: event.starts,
  all_day: event.all_day,
  ...(calendar === undefined ? {} : { calendar }),
});

/** Move the dentist: the undo is the same edit with the values it had. */
const MOVE_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000004061",
  kind: "change",
  state: "pending",
  action: "update_event",
  record: { type: "schedule.event", id: DENTIST.id },
  event: eventFacts(DENTIST, "Campbooks"),
  detail: { starts: MOVED_START, ends: MOVED_END },
  request: {
    operation: "schedule.update_event",
    method: "PATCH",
    path: eventPath(DENTIST),
    body: { starts: MOVED_START, ends: MOVED_END },
  },
  undo: {
    operation: "schedule.update_event",
    method: "PATCH",
    path: eventPath(DENTIST),
    body: { starts: DENTIST.starts, ends: DENTIST.ends },
  },
};

/** Answer the invitation: the undo sends the answer it had. */
const ACCEPT_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000004062",
  kind: "change",
  state: "pending",
  action: "respond_event",
  record: { type: "schedule.event", id: REVIEW.id },
  event: eventFacts(REVIEW, WORK?.name),
  detail: { response: "accepted" },
  request: {
    operation: "schedule.respond_event",
    method: "POST",
    path: eventPath(REVIEW, "/response"),
    body: { response: "accepted" },
  },
  undo: {
    operation: "schedule.respond_event",
    method: "POST",
    path: eventPath(REVIEW, "/response"),
    body: { response: "needs_action" },
  },
};

/** Add the handover to a Google calendar: the undo takes it off, by the id the answer names. */
const PUSH_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000004063",
  kind: "change",
  state: "pending",
  action: "push_event",
  record: { type: "schedule.event", id: HANDOVER.id },
  event: eventFacts(HANDOVER, "Campbooks"),
  detail: { calendar_name: PERSONAL?.name ?? "Personal" },
  request: {
    operation: "schedule.push_event",
    method: "POST",
    path: eventPath(HANDOVER, "/push"),
    body: { calendar_id: PERSONAL?.id ?? "" },
  },
  undo: {
    operation: "schedule.unpush_event",
    method: "DELETE",
    path: "schedule/events/{event_id}/push",
    body: null,
    from_response: ["@event_id:id"],
  },
};

/** Scout's answer to {@link CALENDAR_SAID}, with its three event cards. */
export const CALENDAR_ANSWER: Message = {
  id: "0192f1a0-0000-7000-8000-000000004052",
  author: "scout",
  text:
    "I proposed moving the dentist to Thursday, accepting the design review and adding " +
    "the key handover to your Google calendar. Nothing changes until you confirm each card.",
  steps: [],
  finish: "done",
  proposals: [MOVE_PROPOSAL, ACCEPT_PROPOSAL, PUSH_PROPOSAL],
  created_at: "2026-10-13T14:59:40Z",
};

/** The chat once {@link CALENDAR_SAID} was answered, its cards in `proposals`' states. */
export function chatWithCalendarProposals(
  proposals: readonly Proposal[] = CALENDAR_ANSWER.proposals,
): Chat {
  return {
    ...MARTA_CHAT,
    questions: 1,
    last_message_at: CALENDAR_ANSWER.created_at,
    messages: [
      {
        id: "0192f1a0-0000-7000-8000-000000004051",
        author: "member",
        text: CALENDAR_SAID,
        steps: [],
        finish: null,
        proposals: [],
        created_at: "2026-10-13T14:59:30Z",
      },
      { ...CALENDAR_ANSWER, proposals: [...proposals] },
    ],
  };
}

// Scout proposes a section for a page, or a new page (campbooks-cloud#364):
// cards that say where it goes, who will read it, and the whole text.

const SECTION_ID = "0192f1a0-0000-7000-8000-000000003701";
const KICKOFF_THREAD = { type: "conversations.conversation", id: MARTA.id } as const;

/** The section Scout writes for Acme Studio, in Markdown. */
export const KICKOFF_NOTES = [
  "## Kickoff notes — Acme Studio",
  "",
  "- **Brand files**: sent on 21 Oct (guide and logo pack)",
  "- **Copy sign-off**: still open, nobody has answered",
  "- **First invoice**: Jonas asked to move it to November",
].join("\n");

/** A long section: past twelve lines, so the card folds it behind Show all. */
export const LONG_NOTES = [
  "## Everything agreed with Acme Studio",
  "",
  ...Array.from(
    { length: 14 },
    (_, index) => `- Item ${String(index + 1)}: agreed on 2${String(index % 10)} Oct`,
  ),
  "",
  "Last checked on 28 Oct.",
].join("\n");

const pageFacts = (over: Partial<Schema<"ProposalPage">> = {}): Schema<"ProposalPage"> => ({
  title: "Client onboarding",
  audience: "only_you",
  readers: [],
  others: 0,
  notion: false,
  ...over,
});

/** A section for a page: the request `pages.append` sends, and its undo. */
export function pageProposal(
  over: {
    id?: string;
    page?: Schema<"ProposalPage"> | null;
    markdown?: string | null;
    pageId?: string;
    state?: Proposal["state"];
  } = {},
): Proposal {
  const pageId = over.pageId ?? "0192f1a0-0000-7000-8000-000000000907";
  const markdown = over.markdown === undefined ? KICKOFF_NOTES : over.markdown;
  return {
    id: over.id ?? SECTION_ID,
    kind: "change",
    state: over.state ?? "pending",
    action: "append_section",
    record: { type: "pages.page", id: pageId },
    ...(over.page === null ? {} : { page: over.page ?? pageFacts() }),
    detail: {
      ...(markdown === null ? {} : { markdown }),
      sources: [KICKOFF_THREAD],
    },
    request: {
      operation: "pages.append",
      method: "POST",
      path: `pages/${pageId}/append`,
      body: { markdown: markdown ?? "", sources: [KICKOFF_THREAD] },
    },
    undo: {
      operation: "pages.unappend",
      method: "POST",
      path: `pages/${pageId}/unappend`,
      body: null,
      from_response: ["append_id"],
    },
  };
}

/** A new private page under "Acme Studio". */
export const NEW_PAGE_PROPOSAL: Proposal = {
  id: "0192f1a0-0000-7000-8000-000000003702",
  kind: "change",
  state: "pending",
  action: "new_page",
  page: pageFacts({
    title: "Kickoff notes",
    parent: "Acme Studio",
    audience: "named",
    readers: ["Rui Matos"],
    others: 0,
  }),
  detail: { markdown: KICKOFF_NOTES, sources: [KICKOFF_THREAD] },
  request: {
    operation: "pages.create",
    method: "POST",
    path: "pages",
    body: { title: "Kickoff notes", place: "private", markdown: KICKOFF_NOTES },
  },
  undo: {
    operation: "pages.trash",
    method: "POST",
    path: "pages/{page_id}/trash",
    body: null,
    from_response: ["@page_id:id"],
  },
};
