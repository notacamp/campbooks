// Fictional "what needs you" for tests and stories (never a real person):
// Sam Rivera's needs on Tuesday 13 October 2026, as the attention and
// conversations APIs return them.
import type { Schema } from "~/lib/api";

import { INBOX } from "./conversations";

type Summary = Schema<"ConversationSummary">;
type Need = Schema<"NeedView">;
type Item = Schema<"ItemView">;
type Detail = Schema<"ConversationDetail">;

const base: Pick<
  Summary,
  "inbox_id" | "archived" | "labels" | "suggestion" | "stream" | "stream_source" | "starred"
> = {
  inbox_id: INBOX.id,
  archived: false,
  labels: [],
  suggestion: null,
  stream: "primary",
  stream_source: "default",
  starred: false,
};

interface Spec {
  n: number;
  subject: string;
  from: { name: string; address: string };
  sent: string;
  need: Omit<Need, "message_id" | "quote" | "overdue" | "urgent" | "basis"> & {
    quote: string;
    overdue?: boolean;
  };
  service?: boolean;
}

const hex = (n: number) => n.toString(16).padStart(12, "0");

function conversation(spec: Spec): Summary {
  return {
    ...base,
    id: `0192f1a0-0000-7000-8000-${hex(0x10_00 + spec.n)}`,
    subject: spec.subject,
    last_message_at: spec.sent,
    message_count: 1,
    unread_count: 1,
    latest_from: spec.from,
    snippet: spec.need.quote,
    need: {
      ...spec.need,
      basis: "rules",
      overdue: spec.need.overdue ?? false,
      urgent: false,
      message_id: `0192f1a0-0000-7000-8000-${hex(0x20_00 + spec.n)}`,
      quote: { text: spec.need.quote, from: spec.from, sent_at: spec.sent },
    },
  };
}

export const PHISH = conversation({
  n: 1,
  subject: "Action required: verify your mailbox",
  from: { name: "IT Support", address: "it-support@at3lier-sol.example" },
  sent: "2026-10-13T07:52:00Z",
  need: {
    reason: "review",
    trust: "sure",
    question: "suspicious",
    rule: "signals.lookalike_domain",
    due_on: null,
    quote: "Your mailbox will be closed today unless you verify your account using the link below.",
  },
});

export const MARTA = conversation({
  n: 2,
  subject: "Walkthrough on Thursday",
  from: { name: "Marta Ferreira", address: "marta@casa-lume.example" },
  sent: "2026-10-12T16:40:00Z",
  need: {
    reason: "reply",
    trust: "sure",
    question: "needs_reply",
    rule: "signals.reply_asked",
    due_on: "2026-10-14",
    quote: "Could you confirm Thursday’s walkthrough? The builders need to know by Wednesday.",
  },
});

export const JONAS = conversation({
  n: 3,
  subject: "Lunch on Friday?",
  from: { name: "Jonas Weber", address: "jonas@weber-studio.example" },
  sent: "2026-10-11T19:05:00Z",
  need: {
    reason: "reply",
    trust: "sure",
    question: "needs_reply",
    rule: "signals.question",
    due_on: null,
    quote: "Are you still up for lunch on Friday, or should we move it to next week?",
  },
});

export const PEDRO = conversation({
  n: 4,
  subject: "Lease for the studio",
  from: { name: "Pedro Alves", address: "pedro.alves@example.com" },
  sent: "2026-10-12T10:12:00Z",
  need: {
    reason: "do",
    trust: "sure",
    question: "asks_action",
    rule: "signals.request",
    due_on: "2026-10-16",
    quote: "Please sign the attached lease and send it back by Friday 16 October.",
  },
});

/** A bill the member said is still to pay, two days overdue on Tuesday. */
export const LUZ = conversation({
  n: 5,
  subject: "Invoice 7781",
  from: { name: "Luz Energia", address: "faturas@luz-energia.example" },
  sent: "2026-10-01T09:00:00Z",
  need: {
    reason: "pay",
    trust: "sure",
    question: "money",
    rule: "model.decide",
    due_on: "2026-10-12",
    overdue: true,
    quote: "Please pay invoice 7781 by 12 October to avoid a late fee.",
  },
});

/** A bill Campbooks asks about: "Is it paid?". */
export const ACME = conversation({
  n: 6,
  subject: "Your invoice INV-2041",
  from: { name: "Acme Hosting", address: "billing@acme-hosting.example" },
  sent: "2026-10-13T06:30:00Z",
  need: {
    reason: "pay",
    trust: "check",
    question: "money",
    rule: "model.decide",
    due_on: "2026-10-30",
    quote: "Amount due: €49.00. Payment due by 30 October.",
  },
});

/** Sofia's thread: replied this morning, so its need is gone. */
export const SOFIA: Summary = {
  ...base,
  id: `0192f1a0-0000-7000-8000-${hex(0x10_07)}`,
  subject: "Tile samples",
  last_message_at: "2026-10-13T09:12:00Z",
  message_count: 2,
  unread_count: 0,
  latest_from: { name: "Sam Rivera", address: "sam.rivera@example.com" },
  snippet: "Perfect, thank you!",
  need: null,
};

/** Everything on Home, in the order the attention API returns it. */
export const NEEDS = [PHISH, MARTA, JONAS, PEDRO, LUZ, ACME];

function item(summary: Summary, extra: Partial<Item> = {}): Item {
  const need = summary.need;
  return {
    id: summary.id.replace("8000-00000000", "9000-00000000"),
    record: { type: "conversations.conversation", id: summary.id },
    container: { type: "conversations.inbox", id: INBOX.id },
    cause: { type: "conversations.message", id: need?.message_id ?? summary.id },
    reason: need?.reason ?? "reply",
    trust: need?.trust ?? "sure",
    due_on: need?.due_on ?? null,
    overdue: need?.overdue ?? false,
    urgent: false,
    since: summary.last_message_at,
    state: "open",
    cleared_because: null,
    cleared_by: null,
    cleared_at: null,
    snoozed_until: null,
    ...extra,
  };
}

export const ITEMS: Item[] = NEEDS.map((summary) => item(summary));

export const COUNTS = (["review", "reply", "decide", "do", "pay"] as const).map((reason) => ({
  reason,
  count: ITEMS.filter((i) => i.reason === reason).length,
}));

/** What left the list today: Sofia replied to from the phone. */
export const CLEARED: Item[] = [
  item(SOFIA, {
    reason: "reply",
    state: "cleared",
    cleared_because: "replied",
    cleared_at: "2026-10-13T09:12:00Z",
  }),
];

/** Marta's conversation as the reader (and Home's "How Campbooks decided") reads it. */
export const MARTA_DETAIL: Detail = {
  conversation: MARTA,
  messages: [
    {
      id: MARTA.need?.message_id ?? "",
      direction: "inbound",
      from: { name: "Marta Ferreira", address: "marta@casa-lume.example" },
      to: [{ name: "Sam Rivera", address: "sam.rivera@example.com" }],
      cc: [],
      reply_to: [],
      subject: MARTA.subject,
      sent_at: MARTA.last_message_at,
      read: false,
      starred: false,
      text: {
        content:
          "Hi Sam,\n\nHope the move went well. Could you confirm Thursday’s walkthrough? The builders need to know by Wednesday.\n\nMarta",
        truncated: false,
      },
      has_html: false,
      attachments: [],
    },
  ],
  decisions: [
    {
      question: "needs_reply",
      value: true,
      source: "rules",
      confidence: 90,
      acted: true,
      threshold: 88,
      reasons: [{ rule: "signals.reply_asked", evidence: "could you confirm" }],
    },
    {
      question: "has_deadline",
      value: true,
      source: "rules",
      confidence: 96,
      acted: true,
      threshold: 75,
      reasons: [{ rule: "signals.deadline", evidence: "Wednesday" }],
    },
    {
      question: "suspicious",
      value: false,
      source: "rules",
      confidence: 99,
      acted: false,
      threshold: null,
      reasons: [{ rule: "signals.no_phishing_tell", evidence: "" }],
    },
  ],
};

/** The phishing message's detail: its tells. */
export const PHISH_DETAIL: Detail = {
  conversation: PHISH,
  messages: [],
  decisions: [
    {
      question: "suspicious",
      value: true,
      source: "rules",
      confidence: 93,
      acted: false,
      threshold: null,
      reasons: [
        { rule: "signals.lookalike_domain", evidence: "at3lier-sol.example" },
        { rule: "signals.link_elsewhere", evidence: "verify-mailbox.example" },
      ],
    },
  ],
};
