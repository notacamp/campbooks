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
