// A whole morning on Home for stories and recordings (campbooks-cloud#358):
// Sam Rivera's needs on Tuesday 13 October 2026 across the four priority
// bands, as `attention.list` returns them (the server's order), with the
// conversations and tasks they are about. Fictional people and mail only.
import type { Schema } from "~/lib/api";

import { ACME, ANA, DIGEST, JONAS, MARTA, OFFSITE, PHISH, TIAGO, WATER } from "./attention";
import { INBOX } from "./conversations";
import { BILL, LEASE, taskItem } from "./tasks";

type Summary = Schema<"ConversationSummary">;
type Item = Schema<"ItemView">;

function item(summary: Summary, extra: Partial<Item>): Item {
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

/** The conversations the morning is about (`conversations.list`). */
export const HOME_NEEDS: Summary[] = [
  PHISH,
  ANA,
  MARTA,
  TIAGO,
  WATER,
  JONAS,
  OFFSITE,
  ACME,
  DIGEST,
];

/** The tasks it holds (`tasks.list`). */
export const HOME_TASKS = [BILL, LEASE];

/**
 * Now: the scam warning, the electricity bill two days late. Today: Ana's
 * answer was due Monday, the lease is due today, Marta's walkthrough
 * tomorrow, Tiago has waited four days. This week: the water bill on
 * Thursday, then Jonas and João waiting for an answer. Whenever: Acme's bill
 * on the 30th, and a newsletter a rule keeps here.
 */
export const HOME_ITEMS: Item[] = [
  item(PHISH, { band: "now", band_reason: { rule: "warning" } }),
  taskItem(BILL, { band: "now", band_reason: { rule: "overdue", days: 2 } }),
  item(ANA, { band: "today", band_reason: { rule: "nudge_due", on: "2026-10-12" } }),
  taskItem(LEASE, { band: "today", band_reason: { rule: "due_today" } }),
  item(MARTA, { band: "today", band_reason: { rule: "due_tomorrow" } }),
  item(TIAGO, { band: "today", band_reason: { rule: "waited", days: 4 } }),
  item(WATER, { band: "week", band_reason: { rule: "due_on", on: "2026-10-15" } }),
  item(JONAS, { band: "week", band_reason: { rule: "waiting_answer" } }),
  item(OFFSITE, { band: "week", band_reason: { rule: "waiting_answer" } }),
  item(ACME, { band: "whenever", band_reason: { rule: "due_on", on: "2026-10-30" } }),
  item(DIGEST, { band: "whenever", band_reason: { rule: "kept_by_rule" } }),
];

/** The member's other open tasks, not for today: in Tasks. */
export const HOME_TASKS_ELSEWHERE = 3;

export const HOME_COUNTS = (
  ["review", "reply", "decide", "do", "pay", "nudge", "worth_a_look"] as const
).map((reason) => ({
  reason,
  count: HOME_ITEMS.filter((i) => i.reason === reason).length,
}));
