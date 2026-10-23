// Fictional inbox for tests and stories (never a real person): Sam Rivera's
// mailbox, as the conversations API returns it.
import type { Schema } from "~/lib/api";

import { DUARTE } from "./workspaces";

type Summary = Schema<"ConversationSummary">;
type Detail = Schema<"ConversationDetail">;

export const INBOX = {
  id: "0192f1a0-0000-7000-8000-0000000000b1",
  channel: "email",
  address: "sam.rivera@example.com",
  display_name: "Sam Rivera",
  role: "manager",
  read_only: false,
  unread_count: 3,
} as const;

/** The clock the demo is set at: Tuesday 13 October 2026, 15:00 UTC. */
export const NOW = new Date("2026-10-13T15:00:00Z");

const base: Pick<
  Summary,
  | "inbox_id"
  | "archived"
  | "trashed"
  | "spam"
  | "last_sent_at"
  | "sent_to"
  | "replied_since_sent"
  | "labels"
  | "suggestion"
  | "latest_from_you"
> = {
  inbox_id: INBOX.id,
  archived: false,
  trashed: false,
  spam: false,
  last_sent_at: null,
  sent_to: [],
  replied_since_sent: false,
  labels: [],
  suggestion: null,
  latest_from_you: false,
};

export const Q4: Summary = {
  ...base,
  id: "0192f1a0-0000-7000-8000-0000000000c1",
  subject: "Q4 plan: can you review by Friday?",
  stream: "primary",
  stream_source: "default",
  last_message_at: "2026-10-13T12:00:00Z",
  message_count: 3,
  unread_count: 1,
  starred: false,
  latest_from: { name: "Maya Okafor", address: "maya@northwind-studio.example" },
  snippet: "The timeline can move by two weeks; the price stays the same.",
  labels: [{ id: "0192f1a0-0000-7000-8000-0000000000d1", name: "Clients" }],
  // Sam wrote on Sunday; Maya answered since.
  last_sent_at: "2026-10-11T14:00:00Z",
  sent_to: [{ name: "Maya Okafor", address: "maya@northwind-studio.example" }],
  replied_since_sent: true,
};

export const LEDGER: Summary = {
  ...base,
  id: "0192f1a0-0000-7000-8000-0000000000c2",
  subject: "Five tax dates before year end",
  stream: "primary",
  stream_source: "default",
  last_message_at: "2026-10-12T09:00:00Z",
  message_count: 1,
  unread_count: 1,
  starred: false,
  latest_from: { name: "The Weekly Ledger", address: "hello@weekly-ledger.example" },
  snippet: "Five tax dates before year end, plus what changes for freelancers in 2027.",
  suggestion: {
    stream: "newsletters",
    source: "rules",
    confidence: 95,
    reasons: [
      { rule: "signals.list_header", evidence: "List-Unsubscribe" },
      { rule: "signals.service_sender", evidence: "" },
    ],
  },
};

export const NDA: Summary = {
  ...base,
  id: "0192f1a0-0000-7000-8000-0000000000c3",
  subject: "NDA before Friday’s kickoff",
  stream: "primary",
  stream_source: "default",
  last_message_at: "2026-10-13T09:00:00Z",
  message_count: 1,
  unread_count: 0,
  starred: true,
  latest_from: { name: "Joana Reis", address: "joana@atelier-norte.example" },
  snippet: "Could you send the NDA back signed before Friday?",
};

export const INVOICE: Summary = {
  ...base,
  id: "0192f1a0-0000-7000-8000-0000000000c4",
  subject: "Your invoice CH-20418 for €148.00",
  stream: "receipts",
  stream_source: "member",
  last_message_at: "2026-10-12T13:00:00Z",
  message_count: 1,
  unread_count: 0,
  starred: false,
  latest_from: { name: "Cloudhost Billing", address: "billing@cloudhost.example" },
  snippet: "Your invoice CH-20418 for €148.00 is due on 16 October.",
};

/** A long Portuguese row, for truncation. */
const LONG: Summary = {
  ...base,
  id: "0192f1a0-0000-7000-8000-0000000000c5",
  subject:
    "Re: Re: Fwd: Proposta de alteração ao contrato de prestação de serviços de contabilidade 2027",
  stream: "primary",
  stream_source: "default",
  last_message_at: "2026-10-08T09:41:00Z",
  message_count: 7,
  unread_count: 1,
  starred: false,
  latest_from: {
    name: "Maria do Carmo Sant’Ana Albuquerque e Vasconcelos",
    address: "maria@contabilidade.example",
  },
  snippet: "Segue em anexo a versão revista com as alterações acordadas na reunião",
};

export const PRIMARY = [Q4, NDA, LEDGER, LONG];

/** A conversation Sam started, to three people: archived on arrival, no answer yet. */
export const FIGURES: Summary = {
  ...base,
  id: "0192f1a0-0000-7000-8000-0000000000c6",
  subject: "Figures for the board",
  stream: "primary",
  stream_source: "default",
  last_message_at: "2026-10-12T17:30:00Z",
  message_count: 1,
  unread_count: 0,
  starred: false,
  archived: true,
  latest_from: { name: "Sam Rivera", address: "sam.rivera@example.com" },
  latest_from_you: true,
  snippet: "Here are the figures for Thursday, with last year’s for comparison.",
  last_sent_at: "2026-10-12T17:30:00Z",
  sent_to: [
    { name: "Marta Ferreira", address: "marta@atelier-norte.example" },
    { name: "Leo Brandt", address: "leo@northwind-studio.example" },
    { address: "accounts@atelier-norte.example" },
  ],
};

/** Sent: by Sam's latest message. */
export const SENT = [FIGURES, Q4];

/** Reported as spam: binned, in Spam rather than in Trash. */
export const SPAMMED: Summary = {
  ...LEDGER,
  id: "0192f1a0-0000-7000-8000-0000000000c7",
  subject: "You have won a cruise",
  latest_from: { name: "Prize Desk", address: "win@prize-desk.example" },
  snippet: "Claim your cruise before midnight.",
  suggestion: null,
  archived: true,
  trashed: true,
  spam: true,
};

const SAM = { name: "Sam Rivera", address: "sam.rivera@example.com" };
const MAYA = { name: "Maya Okafor", address: "maya@northwind-studio.example" };

export const Q4_DETAIL: Detail = {
  conversation: Q4,
  messages: [
    {
      id: "0192f1a0-0000-7000-8000-0000000000e1",
      direction: "inbound",
      from: MAYA,
      to: [SAM],
      cc: [{ name: "Leo Brandt", address: "leo@northwind-studio.example" }],
      reply_to: [],
      subject: Q4.subject,
      sent_at: "2026-10-11T11:00:00Z",
      read: true,
      starred: false,
      text: {
        content: "Hi Sam, here is the first draft of the Q4 plan. Could you review it by Friday?",
        truncated: false,
      },
      has_html: false,
      attachments: [
        {
          id: "0192f1a0-0000-7000-8000-0000000000f1",
          filename: "q4-plan-draft.pdf",
          content_type: "application/pdf",
          size: 48_000,
          inline: false,
          in_body: false,
        },
      ],
    },
    {
      id: "0192f1a0-0000-7000-8000-0000000000e2",
      direction: "outbound",
      from: SAM,
      to: [MAYA],
      cc: [],
      reply_to: [],
      subject: `Re: ${Q4.subject}`,
      sent_at: "2026-10-11T14:00:00Z",
      read: true,
      starred: false,
      text: {
        content: "Thanks Maya, I will look at it this week. Is the timeline fixed?",
        truncated: false,
      },
      has_html: false,
      attachments: [],
    },
    {
      id: "0192f1a0-0000-7000-8000-0000000000e3",
      direction: "inbound",
      from: MAYA,
      to: [SAM],
      cc: [],
      reply_to: [],
      subject: `Re: ${Q4.subject}`,
      sent_at: "2026-10-13T12:00:00Z",
      read: false,
      starred: false,
      text: {
        content:
          "The timeline can move by two weeks; the price stays the same. Attached is version 3.",
        truncated: false,
      },
      has_html: true,
      attachments: [
        {
          id: "0192f1a0-0000-7000-8000-0000000000f2",
          filename: "q4-plan-v3.pdf",
          content_type: "application/pdf",
          size: 52_000,
          inline: false,
          in_body: false,
        },
      ],
    },
  ],
  decisions: [
    {
      question: "stream",
      value: "primary",
      source: "rules",
      confidence: 90,
      acted: false,
      reasons: [{ rule: "signals.person_sender", evidence: "" }],
    },
    {
      question: "needs_reply",
      value: true,
      source: "rules",
      confidence: 80,
      acted: false,
      reasons: [{ rule: "signals.question", evidence: "?" }],
    },
    {
      question: "has_deadline",
      value: true,
      source: "rules",
      confidence: 75,
      acted: false,
      reasons: [{ rule: "signals.deadline", evidence: "Friday" }],
    },
  ],
};

export const LEDGER_DETAIL: Detail = {
  conversation: LEDGER,
  messages: [
    {
      id: "0192f1a0-0000-7000-8000-0000000000e4",
      direction: "inbound",
      from: { name: "The Weekly Ledger", address: "hello@weekly-ledger.example" },
      to: [SAM],
      cc: [],
      reply_to: [],
      subject: LEDGER.subject,
      sent_at: LEDGER.last_message_at,
      read: false,
      starred: false,
      text: { content: LEDGER.snippet, truncated: false },
      has_html: true,
      attachments: [],
    },
  ],
  decisions: [
    {
      question: "stream",
      value: "newsletters",
      source: "rules",
      confidence: 95,
      acted: false,
      reasons: LEDGER.suggestion?.reasons ?? [],
    },
  ],
};

/** The Q4 reply's body, as the server sanitises it. */
export const Q4_BODY = {
  text: Q4_DETAIL.messages[2]?.text?.content ?? "",
  html: "<p>The timeline can move by two weeks; the price stays the same.</p><p>Attached is <b>version 3</b>.</p>",
  truncated: false,
  remote_images: 0,
  images_shown: false,
  images_trusted: false,
  images_offer: false,
  images_domain: null,
};

/** The newsletter's body: two remote images held back until "Show images". */
export function ledgerBody(shown: boolean) {
  const image = (src: string, alt: string) =>
    shown
      ? `<img src="${src}" alt="${alt}" width="560" height="120">`
      : `<img alt="${alt}" width="560" height="120">`;
  return {
    text: LEDGER.snippet,
    html: `${image("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='560' height='120'%3E%3Crect width='560' height='120' fill='%232f5d50'/%3E%3C/svg%3E", "The Weekly Ledger")}<h1 style="font-size: 24px">Five tax dates before year end</h1><p>Mark these in your calendar, and read <a href="https://weekly-ledger.example/articles" target="_blank" rel="noopener noreferrer">what changes in 2027</a>.</p>`,
    truncated: false,
    remote_images: 1,
    images_shown: shown,
    images_trusted: false,
    // An authenticated newsletter no rule trusts yet: trusting it may be offered.
    images_offer: !shown,
    images_domain: shown ? null : "weekly-ledger.example",
  };
}

export const STREAM_COUNTS = [
  { stream: "primary", conversations: 4, unread: 3 },
  { stream: "updates", conversations: 0, unread: 0 },
  { stream: "receipts", conversations: 1, unread: 0 },
  { stream: "newsletters", conversations: 0, unread: 0 },
] as const;

/** The API paths of the demo workspace, behind the BFF. */
export const W = `/bff/api/v1/w/${DUARTE.id}`;

/** The Ledger rule's run over existing mail, finished. */
export const LEDGER_RUN: NonNullable<Schema<"RuleView">["run"]> = {
  id: "0192f1a0-0000-7000-8000-0000000000e3",
  kind: "apply",
  state: "done",
  total: 40,
  counted: true,
  done: 40,
  changed: 40,
  skipped_suspicious: 0,
  skipped_member: 0,
  started_at: "2026-10-13T15:00:00Z",
  finished_at: "2026-10-13T15:00:05Z",
};

/** A taught rule moving the Weekly Ledger's mail to Newsletters (teach once). */
export const LEDGER_RULE: Schema<"RuleView"> = {
  id: "0192f1a0-0000-7000-8000-0000000000e1",
  inbox_id: INBOX.id,
  teaching_id: "0192f1a0-0000-7000-8000-0000000000e2",
  conditions: [{ field: "sender_address", value: "hello@weekly-ledger.example" }],
  actions: [{ action: "move_to_stream", stream: "newsletters" }],
  origin: "action",
  origin_text: null,
  state: "active",
  taught_by: "0192f1a0-0000-7000-8000-0000000000a1",
  from_conversation_id: "0192f1a0-0000-7000-8000-0000000000c2",
  created_at: "2026-10-13T15:00:00Z",
  confirmed_at: "2026-10-13T15:00:00Z",
  applied_count: 40,
  last_applied_at: "2026-10-13T15:00:00Z",
  recent_exceptions: 0,
  run: LEDGER_RUN,
  shared: false,
  may_change: true,
};

/** What `conversations.suggest_rules` offers after the Ledger is moved. */
export const LEDGER_SUGGESTIONS: Schema<"RuleSuggestions"> = {
  shared: false,
  default: 0,
  none: null,
  suggestions: [
    {
      likeness: "sender",
      key: "sender|ledger",
      conditions: [{ field: "sender_address", value: "hello@weekly-ledger.example" }],
      actions: [{ action: "move_to_stream", stream: "newsletters" }],
      preview: {
        matching: 41,
        would_change: 40,
        skipped_suspicious: 0,
        skipped_member: 0,
        overrides: [],
        complete: true,
        samples: [
          {
            conversation_id: "0192f1a0-0000-7000-8000-0000000000d1",
            subject: "What changes for freelancers in 2027",
            sender: "hello@weekly-ledger.example",
            sent_at: "2026-10-05T09:00:00Z",
          },
        ],
      },
    },
  ],
};

/**
 * What Scout noticed in Sam's inbox (campbooks-cloud#169): the build robot
 * of a fictional CI service writes most days, and Sam never opens it.
 */
export const BUILDS_NOTICE: Schema<"RuleNotice"> = {
  id: "0192f1a0-0000-7000-8000-0000000000e7",
  inbox_id: INBOX.id,
  kind: "keep_off_home",
  sender: "builds@ci.example.com",
  sender_name: "CI",
  conditions: [{ field: "sender_address", value: "builds@ci.example.com" }],
  actions: [
    { action: "move_to_stream", stream: "updates" },
    { action: "surface", value: "never" },
  ],
  evidence: {
    received: 34,
    opened: 2,
    archived_unread: 0,
    last_week: 7,
    window_days: 60,
    cadence: "daily",
  },
  shared: false,
};
