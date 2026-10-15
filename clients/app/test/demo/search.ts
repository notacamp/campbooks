// Fictional search results for tests and stories (never a real person): what
// `search.query` returns for "invoice", as the conversations in demo/conversations.
import type { Schema } from "~/lib/api";

import { INBOX, INVOICE, LEDGER, Q4 } from "./conversations";

type Hit = Schema<"Hit">;
type Results = Schema<"SearchResults">;

const record = (id: string) => ({ type: "conversations.conversation", id });
const miss = (text: string) => ({ text, hit: false });
const hit = (text: string) => ({ text, hit: true });

export const INVOICE_HIT: Hit = {
  record: record(INVOICE.id),
  container: { type: "conversations.inbox", id: INBOX.id },
  title: [miss("Your "), hit("invoice"), miss(" CH-20418 for €148.00")],
  snippet: [miss("Your "), hit("invoice"), miss(" is due on 16 October.")],
  from: INVOICE.latest_from ?? null,
  occurred_at: INVOICE.last_message_at,
  facets: { stream: "receipts", unread: false, starred: false, count: 1, attachment: true },
};

export const LEDGER_HIT: Hit = {
  record: record(LEDGER.id),
  container: { type: "conversations.inbox", id: INBOX.id },
  title: [miss("Five tax dates before year end")],
  snippet: [miss("…send your "), hit("invoices"), miss(" before the 31st.")],
  from: LEDGER.latest_from ?? null,
  occurred_at: LEDGER.last_message_at,
  facets: { stream: "newsletters", unread: true, starred: false, count: 1 },
};

export const Q4_HIT: Hit = {
  record: record(Q4.id),
  container: { type: "conversations.inbox", id: INBOX.id },
  title: [miss("Q4 plan: can you review by Friday?")],
  snippet: [miss("The "), hit("invoice"), miss(" schedule is unchanged.")],
  from: Q4.latest_from ?? null,
  occurred_at: Q4.last_message_at,
  facets: { stream: "primary", unread: true, starred: false, count: 3 },
};

export const results = (hits: Hit[], extra: Partial<Results> = {}, text = "invoice"): Results => ({
  hits,
  indexing: false,
  next_cursor: null,
  query: { text, filters: [], warnings: [] },
  ...extra,
});
