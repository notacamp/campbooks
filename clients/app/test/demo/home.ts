// A whole morning on Home for stories and recordings (campbooks-cloud#358):
// Sam Rivera's needs on Tuesday 13 October 2026 across the four priority
// bands, as `attention.list` returns them (the server's order), with the
// conversations and tasks they are about, and their calendar
// (campbooks-cloud#359). Fictional people and mail only.
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

/* The calendar (campbooks-cloud#359). */

type Event = Schema<"EventView">;
type Calendar = Schema<"CalendarView">;
type Task = Schema<"TaskView">;

/**
 * Sam's calendar is in New York time this week: the demo's 15:00 UTC is
 * 11:00 there, so the morning still has its meetings ahead.
 */
const HOME_ZONE = "America/New_York";

const calendarId = (n: number) => `0192f1a0-0000-7000-8000-0000000046${String(n).padStart(2, "0")}`;
const eventId = (n: number) => `0192f1a0-0000-7000-8000-0000000047${String(n).padStart(2, "0")}`;

/** What Scout finds in Sam's mail: only Sam sees it until they add it to another calendar. */
const OWN: Calendar = {
  id: calendarId(1),
  name: "Campbooks",
  source: "campbooks",
  primary: false,
  syncing: true,
  writable: true,
  color: null,
  time_zone: HOME_ZONE,
};
const WORK: Calendar = {
  id: calendarId(2),
  name: "Work",
  source: "provider",
  provider: "google",
  primary: true,
  syncing: true,
  writable: true,
  color: "#2f9e6b",
  time_zone: HOME_ZONE,
};

export const HOME_CALENDARS = [OWN, WORK];

export const HOME_SETTINGS: Schema<"SettingsView"> = {
  time_zone: HOME_ZONE,
  effective_time_zone: HOME_ZONE,
  scout_kinds: ["travel", "appointment", "meeting", "delivery", "charge", "expiry"],
  also_add_to: {},
  last_push_calendar: WORK.id,
  working_hours: Object.fromEntries(
    ["mon", "tue", "wed", "thu", "fri"].map((day) => [day, [{ start: "09:00", end: "18:00" }]]),
  ),
};

const event = (
  n: number,
  fields: Pick<Event, "kind" | "title" | "starts" | "ends"> & Partial<Event>,
): Event => ({
  id: eventId(n),
  calendar_id: WORK.id,
  all_day: false,
  status: "confirmed",
  recurring: false,
  origin: "provider",
  pending: false,
  on_provider: true,
  busy: true,
  item_keys: [],
  ...fields,
});

/** Read by Scout from mail, on Sam's Campbooks calendar only. */
const found = (
  n: number,
  fields: Pick<Event, "kind" | "title" | "starts" | "ends"> & Partial<Event>,
): Event =>
  event(n, {
    calendar_id: OWN.id,
    origin: "scout",
    on_provider: false,
    found_by: "structured",
    ...fields,
  });

/** In 20 minutes, with a web address to join at. */
const HOME_CALL = event(1, {
  kind: "meeting",
  title: "Kitchen plan call with Marta Ferreira",
  starts: "2026-10-13T11:20:00-04:00",
  ends: "2026-10-13T11:50:00-04:00",
  place: "https://meet.example.com/kxr-4471-tmp",
  response: "accepted",
});

const HOME_LUNCH = event(2, {
  kind: "reservation",
  title: "Lunch with Rui",
  starts: "2026-10-13T12:30:00-04:00",
  ends: "2026-10-13T13:30:00-04:00",
  place: "Tasca do Bairro",
});

/** Time Sam held for the lease task. */
const HOME_HOLD = found(3, {
  kind: "hold",
  title: "Send the signed lease to Marta",
  starts: "2026-10-13T15:00:00-04:00",
  ends: "2026-10-13T15:30:00-04:00",
  origin: "member",
  found_by: null,
  busy: true,
});

/** Read from Marta's email: only Sam sees it. */
const HOME_HANDOVER = found(4, {
  kind: "appointment",
  title: "Key handover · Rua das Flores 12",
  starts: "2026-10-13T18:00:00-04:00",
  ends: "2026-10-13T18:30:00-04:00",
  place: "Rua das Flores 12",
  source: { type: "conversations.message", id: MARTA.need?.message_id ?? "" },
  source_conversation: MARTA.id,
});

/** Declined: not on Home. */
const HOME_DECLINED = event(5, {
  kind: "meeting",
  title: "Supplier demo",
  starts: "2026-10-13T16:00:00-04:00",
  ends: "2026-10-13T16:30:00-04:00",
  response: "declined",
});

export const HOME_DELIVERY = found(6, {
  kind: "delivery",
  title: "Furniture delivery, 2 of 3 parcels",
  starts: "2026-10-14T09:00:00-04:00",
  ends: "2026-10-14T13:00:00-04:00",
});

const HOME_CHARGE = event(7, {
  kind: "charge",
  title: "Streaming subscription · €13.99",
  all_day: true,
  starts: "2026-10-15",
  ends: "2026-10-16",
  busy: false,
});

const HOME_RENEWAL = event(8, {
  kind: "expiry",
  title: "Domain casa-rivera.example renews · €12.30",
  all_day: true,
  starts: "2026-10-17",
  ends: "2026-10-18",
  busy: false,
});

/** A train only Sam sees, read from the ticket's email. */
const HOME_TRAIN = found(9, {
  kind: "travel",
  title: "Train to Boston, car 4 seat 52",
  starts: "2026-10-18T08:15:00-04:00",
  ends: "2026-10-18T12:20:00-04:00",
});

/** Today and the week after it, as `schedule.list_events` answers. */
export const HOME_EVENTS: Event[] = [
  HOME_CALL,
  HOME_LUNCH,
  HOME_HOLD,
  HOME_DECLINED,
  HOME_HANDOVER,
  HOME_DELIVERY,
  HOME_CHARGE,
  HOME_RENEWAL,
  HOME_TRAIN,
];

/** A task due Friday: Coming up's Due line. */
export const HOME_DUE: Task = {
  ...LEASE,
  id: "0192f1a0-0000-7000-8000-0000000007f1",
  title: "Review Marta’s proposal draft",
  due_on: "2026-10-16",
  source: null,
};
