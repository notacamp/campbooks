// Fictional drafts for tests and stories (never a real person): Sam Rivera
// answering Maya Okafor, as the drafts API returns them.
import type { Schema } from "~/lib/api";

import { INBOX, Q4, Q4_DETAIL } from "./conversations";

type Draft = Schema<"DraftView">;

const [FIRST] = Q4_DETAIL.messages;

/** Sam's reply to Maya's first message, just started. */
export const REPLY: Draft = {
  id: "0192f1a0-0000-7000-8000-000000000101",
  inbox_id: INBOX.id,
  kind: "reply",
  conversation_id: Q4.id,
  message_id: FIRST?.id ?? null,
  from: { name: INBOX.display_name, address: INBOX.address },
  to: [{ name: "Maya Okafor", address: "maya@northwind-studio.example" }],
  cc: [],
  bcc: [],
  subject: `Re: ${Q4.subject}`,
  body: "",
  quoted: "On 11 Oct 2026, 11:00, Maya Okafor wrote:\n> Could you review it by Friday?",
  attachments: [],
  version: 1,
  state: "draft",
  send_at: null,
  failure_reason: null,
  created_at: "2026-10-13T14:00:00Z",
  updated_at: "2026-10-13T14:00:00Z",
  content_token: "demo-token-reply",
};

/** A new message, still empty. */
export const NEW: Draft = {
  ...REPLY,
  id: "0192f1a0-0000-7000-8000-000000000102",
  kind: "new",
  conversation_id: null,
  message_id: null,
  to: [],
  subject: "",
  quoted: "",
};

/** Sam's reply to Maya, half written. */
export const REPLY_WRITTEN: Draft = {
  ...REPLY,
  body: "Thanks, Maya. I will go through the plan on Thursday and send you my notes before the end of the day.",
  updated_at: "2026-10-13T14:30:00Z",
};

/** A new message to Marta Ferreira and two others. */
export const TO_MARTA: Draft = {
  ...NEW,
  id: "0192f1a0-0000-7000-8000-000000000103",
  to: [
    { name: "Marta Ferreira", address: "marta@example.com" },
    { name: "Joana Reis", address: "joana@atelier-norte.example" },
  ],
  cc: [{ name: "Sam Rivera", address: "sam@example.com" }],
  subject: "Figures for the board",
  body: "Marta,\n\nThe figures are in the folder. Two lines still need your eye.",
  updated_at: "2026-10-12T16:00:00Z",
};

/** A new message with nothing in it yet. */
export const BLANK: Draft = {
  ...NEW,
  id: "0192f1a0-0000-7000-8000-000000000104",
  updated_at: "2026-10-10T08:00:00Z",
};

/** What `conversations.list_drafts` answers, newest first. */
export const DRAFT_LIST: Draft[] = [REPLY_WRITTEN, TO_MARTA, BLANK];
