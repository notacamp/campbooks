// Fictional tasks for tests and stories (never a real person): Ana Duarte's
// list on Tuesday 13 October 2026, as the tasks API returns it.
import type { Schema } from "~/lib/api";

import { LUZ, MARTA } from "./attention";
import { ANA, RUI, SOFIA } from "./workspaces";

type Task = Schema<"TaskView">;

/** The member's Tuesday afternoon, in their own time zone. */
export const TASKS_NOW = new Date(2026, 9, 13, 15, 0);

const task = (n: number, over: Partial<Task>): Task => ({
  id: `0192f1a0-0000-7000-8000-0000000007${n.toString(16).padStart(2, "0")}`,
  title: "",
  verb: "do",
  origin: "own",
  due_on: null,
  due_at: null,
  overdue: false,
  plan: "none",
  plan_on: null,
  later_until: null,
  state: "open",
  closed_at: null,
  assignee: ANA.user_id,
  created_by: ANA.user_id,
  handed_by: null,
  source: null,
  asks_done: false,
  revision: 1,
  created_at: "2026-10-10T09:00:00Z",
  updated_at: "2026-10-10T09:00:00Z",
  ...over,
});

/** A bill two days late. */
export const BILL = task(1, {
  title: "EDP electricity bill · €84.20",
  verb: "pay",
  due_on: "2026-10-11",
  overdue: true,
});

/** Made from Marta's email, due today. */
export const LEASE = task(2, {
  title: "Send the signed lease to Marta",
  due_on: "2026-10-13",
  source: { type: "conversations.conversation", id: MARTA.id },
});

/** No due date, planned for today. */
export const ACCOUNTANT = task(3, {
  title: "Call the accountant about Q3 VAT",
  plan: "date",
  plan_on: "2026-10-13",
});

export const PROPOSAL = task(4, {
  title: "Review Sofia’s proposal draft",
  due_on: "2026-10-18",
});

export const PHOTOS = task(5, { title: "Send Rui the photos from Sunday" });

/** Put off until the first of November. */
export const INSPECTION = task(6, {
  title: "Book the car inspection",
  plan: "later",
  later_until: "2026-11-02T09:00:00Z",
});

/** Handed to Rui Matos; still Ana's to take back. */
export const VENUE = task(7, {
  title: "Book the venue for the team lunch",
  due_on: "2026-10-20",
  assignee: RUI.user_id,
  handed_by: ANA.user_id,
});

/** Scout found it in Marta's email: the signed lease, due Friday. */
export const ASK_LEASE = task(8, {
  title: "Send the signed lease",
  origin: "ask",
  due_on: "2026-10-16",
  source: { type: "conversations.conversation", id: MARTA.id },
});

/**
 * The lease task after Ana replied to Marta's email (campbooks-cloud#396):
 * Scout asks whether it is done. Not yet is the same task with `asks_done`
 * off.
 */
export const ASK_REPLIED = task(10, {
  title: "Send the signed lease",
  origin: "ask",
  due_on: "2026-10-16",
  source: { type: "conversations.conversation", id: MARTA.id },
  asks_done: true,
  replied: { at: "2026-10-13T14:20:00Z", message: "0192f1a0-0000-7000-8000-00000000aa01" },
  revision: 2,
});

/** Scout found it in the energy company's invoice, late. */
export const ASK_BILL = task(9, {
  title: "Pay invoice 7781 · €84.20",
  verb: "pay",
  origin: "ask",
  due_on: "2026-10-12",
  overdue: true,
  source: { type: "conversations.conversation", id: LUZ.id },
});

/** Ana's open list, as `tasks.list` answers it. */
export const OPEN_TASKS = [BILL, LEASE, ACCOUNTANT, PROPOSAL, PHOTOS, INSPECTION];

export const TEAM = [ANA, SOFIA, RUI];

/** The attention item Home holds for a task, as `attention.list` answers. */
export function taskItem(from: Task, extra: Partial<Schema<"ItemView">> = {}): Schema<"ItemView"> {
  return {
    id: from.id.replace("8000-00000000", "9000-00000000"),
    record: { type: "tasks.task", id: from.id },
    container: { type: "tasks.assignee", id: ANA.user_id },
    cause: from.source ?? { type: "tasks.task", id: from.id },
    reason: from.verb,
    trust: "sure",
    due_on: from.due_on ?? null,
    overdue: from.overdue,
    urgent: false,
    since: "2026-10-10T09:00:00Z",
    state: "open",
    cleared_because: null,
    cleared_by: null,
    cleared_at: null,
    snoozed_until: null,
    ...extra,
  };
}
