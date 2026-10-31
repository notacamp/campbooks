// "Kickoff agenda", the thread Scout explains in the stories and the
// recording (campbooks-cloud#407): Jonas Weber of Acme Studio, Rui Matos and
// Ana Duarte, whose inbox it is. Fictional, never a real person.
import type { Schema } from "~/lib/api";

import { INBOX } from "./conversations";

type Summary = Schema<"ConversationSummary">;
type Detail = Schema<"ConversationDetail">;
type Message = Schema<"MessageView">;
export type Explained = Schema<"ExplanationView">;

/** Ana's inbox: the one the thread is in. */
export const ANAS_INBOX = {
  ...INBOX,
  address: "ana@example.com",
  display_name: "Ana Duarte",
};

const JONAS = { name: "Jonas Weber", address: "jonas@acme-studio.example" };
const RUI = { name: "Rui Matos", address: "rui@example.com" };
const ANA = { name: "Ana Duarte", address: "ana@example.com" };

const TEXTS: [typeof JONAS, string][] = [
  [
    JONAS,
    "Hi Ana, here is a first agenda for the kickoff. Can we do two workshops instead of one?",
  ],
  [RUI, "Two workshops works for us. Thursday the 6th at 10 works too, at your office."],
  [ANA, "Thursday the 6th at 10 is fine. I will send the revised quote."],
  [JONAS, "Great. Could you send a revised quote that includes the second workshop?"],
  [RUI, "Ana, can you send me who is coming from our side by Monday?"],
  [JONAS, "What would the second workshop cost? Nobody has answered that yet."],
  [RUI, "I will check whether our lawyer joins and confirm."],
  [JONAS, "Thank you both. I will confirm whether Acme Studio's lawyer joins."],
  [RUI, "Ana, the list of who is coming, by Monday please."],
];

const id = (n: number) => `0192f1a0-0000-7000-8000-00000000${String(7000 + n)}`;
const day = (n: number) =>
  `2026-10-${String(20 + Math.floor(n / 2)).padStart(2, "0")}T${String(9 + (n % 3)).padStart(2, "0")}:00:00Z`;

function message(n: number, from: typeof JONAS, text: string): Message {
  const yours = from.address === ANA.address;
  return {
    id: id(n),
    direction: yours ? "outbound" : "inbound",
    from,
    to: [yours ? JONAS : ANA],
    cc: [],
    reply_to: [],
    subject: "Kickoff agenda",
    sent_at: day(n),
    read: true,
    starred: false,
    text: { content: text, truncated: false },
    has_html: false,
    attachments: [],
  };
}

const KICKOFF_MESSAGES: Message[] = TEXTS.map(([from, text], at) => message(at + 1, from, text));

/** Rui's message after the explanation was written (the thread moved on). */
const KICKOFF_NEWER: Message = {
  ...message(10, RUI, "Jonas, our lawyer can join too if yours does."),
  sent_at: "2026-10-26T11:05:00Z",
};

function summary(messages: Message[]): Summary {
  const last = messages.at(-1);
  return {
    id: "0192f1a0-0000-7000-8000-0000000007a0",
    inbox_id: ANAS_INBOX.id,
    subject: "Kickoff agenda",
    stream: "primary",
    stream_source: "default",
    last_message_at: last?.sent_at ?? "2026-10-25T11:00:00Z",
    message_count: messages.length,
    unread_count: 0,
    starred: false,
    archived: false,
    trashed: false,
    spam: false,
    last_sent_at: null,
    sent_to: [],
    replied_since_sent: false,
    labels: [],
    suggestion: null,
    latest_from_you: false,
    latest_from: last?.from ?? RUI,
    snippet: last?.text?.content ?? "",
  };
}

export const KICKOFF = summary(KICKOFF_MESSAGES);
export const KICKOFF_STALE = summary([...KICKOFF_MESSAGES, KICKOFF_NEWER]);

export function kickoffDetail(options: { newer?: boolean; explain?: boolean } = {}): Detail {
  const messages = options.newer === true ? [...KICKOFF_MESSAGES, KICKOFF_NEWER] : KICKOFF_MESSAGES;
  return {
    conversation: options.newer === true ? KICKOFF_STALE : KICKOFF,
    messages,
    decisions: [],
    ...(options.explain === false ? {} : { explain: true }),
  };
}

const ref = (n: number, from: typeof JONAS) => ({
  id: id(n),
  from: from.name,
  sent_at: day(n),
});

/** Scout's explanation of the nine messages, as `conversations.get_explanation` answers it. */
export function kickoffExplained(overrides: Partial<Explained> = {}): Explained {
  return {
    state: "ready",
    locale: "en",
    written_at: "2026-10-25T10:42:00Z",
    messages: 9,
    in_full: 9,
    stale: false,
    body: {
      in_short:
        "Acme Studio wants the kickoff on Thursday 6 November and a revised quote before then. Rui agreed to the date; the quote is still with you.",
      wants: [
        {
          who: "Jonas Weber",
          what: "The kickoff on Thu 6 Nov at 10:00, and a revised quote that includes the second workshop.",
          refs: [id(4)],
        },
        { who: "Rui Matos", what: "The list of who is coming from your side.", refs: [id(5)] },
      ],
      agreed: [
        { text: "Kickoff on Thu 6 Nov, 10:00, at your office.", refs: [id(2)] },
        { text: "Two workshops instead of one.", refs: [id(1)] },
      ],
      open: [
        {
          text: "The price of the second workshop: Jonas asked and nobody has answered yet.",
          refs: [id(6)],
        },
        { text: "Whether Acme Studio's lawyer joins: Jonas said he would confirm.", refs: [id(8)] },
      ],
      on_you: [
        { text: "Send Jonas the revised quote.", due_on: "2026-11-06", refs: [id(4)] },
        { text: "Send Rui the list of who is coming.", due_on: "2026-11-03", refs: [id(9)] },
      ],
    },
    refs: [
      ref(1, JONAS),
      ref(2, RUI),
      ref(4, JONAS),
      ref(5, RUI),
      ref(6, JONAS),
      ref(8, JONAS),
      ref(9, RUI),
    ],
    ...overrides,
  };
}
