// Fictional calendar for tests and stories (never a real person): Ana
// Duarte's week of Tuesday 27 October 2026, as the schedule API returns it.
// Times are written in her zone, Europe/Lisbon (UTC+0 once summer time ended
// on Sunday 25 October); an all-day event is a plain date, its end exclusive.
import type { Schema } from "~/lib/api";

import { INBOX, Q4 } from "./conversations";

type Event = Schema<"EventView">;
type Calendar = Schema<"CalendarView">;
type Summary = Schema<"ConversationSummary">;
type Detail = Schema<"ConversationDetail">;
type Read = Schema<"MessageReadView">;

/** The clock the schedule demo is set at: Tuesday 27 October 2026, 14:20 in Lisbon. */
export const SCHEDULE_NOW = "2026-10-27T14:20:00+00:00";
const SCHEDULE_ZONE = "Europe/Lisbon";

const id = (n: number) => `0192f1a0-0000-7000-8000-0000000042${String(n).padStart(2, "0")}`;

export const CAMPBOOKS: Calendar = {
  id: id(1),
  name: "Campbooks",
  source: "campbooks",
  primary: false,
  syncing: true,
  writable: true,
  color: null,
  time_zone: SCHEDULE_ZONE,
};
const PERSONAL: Calendar = {
  id: id(2),
  name: "Personal",
  source: "provider",
  provider: "google",
  primary: true,
  syncing: true,
  writable: true,
  color: "#2b7fb8",
  time_zone: SCHEDULE_ZONE,
};
const WORK: Calendar = {
  id: id(3),
  name: "Work",
  source: "provider",
  provider: "google",
  primary: false,
  syncing: true,
  writable: true,
  color: "#2f9e6b",
  time_zone: SCHEDULE_ZONE,
};
export const HOLIDAYS: Calendar = {
  id: id(4),
  name: "Holidays in Portugal",
  source: "provider",
  provider: "google",
  primary: false,
  syncing: false,
  writable: false,
  color: "#b88a2b",
  time_zone: SCHEDULE_ZONE,
};
export const CALENDARS = [CAMPBOOKS, PERSONAL, WORK, HOLIDAYS];

export const SETTINGS: Schema<"SettingsView"> = {
  time_zone: SCHEDULE_ZONE,
  effective_time_zone: SCHEDULE_ZONE,
  scout_kinds: [
    "travel",
    "stay",
    "show",
    "reservation",
    "appointment",
    "meeting",
    "delivery",
    "charge",
  ],
  also_add_to: { travel: PERSONAL.id, show: PERSONAL.id },
  last_push_calendar: PERSONAL.id,
  working_hours: Object.fromEntries(
    ["mon", "tue", "wed", "thu", "fri"].map((day) => [day, [{ start: "09:00", end: "18:00" }]]),
  ),
};

/** The emails the events were read from. */
const message = (n: number) => `0192f1a0-0000-7000-8000-0000000043${String(n).padStart(2, "0")}`;
const conversation = (n: number) =>
  `0192f1a0-0000-7000-8000-0000000044${String(n).padStart(2, "0")}`;

const base: Pick<
  Event,
  | "busy"
  | "on_provider"
  | "origin"
  | "pending"
  | "recurring"
  | "status"
  | "all_day"
  | "calendar_id"
  | "item_keys"
> = {
  busy: true,
  item_keys: [],
  on_provider: false,
  origin: "scout",
  pending: false,
  recurring: false,
  status: "confirmed",
  all_day: false,
  calendar_id: CAMPBOOKS.id,
};

/** An event Scout read from message `n`. */
function scout(
  n: number,
  fields: Pick<Event, "id" | "kind" | "title" | "starts" | "ends"> & Partial<Event>,
): Event {
  return {
    ...base,
    found_by: "structured",
    source: { type: "conversations.message", id: message(n) },
    source_conversation: conversation(n),
    ...fields,
  };
}

const SYNC: Event = {
  ...base,
  id: id(10),
  kind: "meeting",
  title: "Weekly sync with Rui Costa",
  starts: "2026-10-27T09:30:00+00:00",
  ends: "2026-10-27T10:00:00+00:00",
  origin: "provider",
  calendar_id: WORK.id,
  on_provider: true,
  recurring: true,
  response: "accepted",
  place: "Video call",
};
const STANDUP: Event = {
  ...base,
  id: id(11),
  kind: "meeting",
  title: "Standup",
  starts: "2026-10-26T09:30:00+00:00",
  ends: "2026-10-26T10:00:00+00:00",
  origin: "provider",
  calendar_id: WORK.id,
  on_provider: true,
  recurring: true,
};
const HOLD: Event = {
  ...base,
  id: id(12),
  kind: "hold",
  title: "Held · Quote for Marta Lopes",
  starts: "2026-10-27T11:00:00+00:00",
  ends: "2026-10-27T12:30:00+00:00",
  origin: "member",
  calendar_id: WORK.id,
  on_provider: true,
  source: { type: "tasks.task", id: id(90) },
};
export const DENTIST: Event = scout(1, {
  id: id(13),
  kind: "appointment",
  title: "Dentist check-up",
  starts: "2026-10-27T15:00:00+00:00",
  ends: "2026-10-27T15:45:00+00:00",
  place: "Clínica Sorriso, Rua Augusta 120, Lisboa",
  found_by: "model",
  item_keys: [`${message(1)}:model:item:0`],
});
const NUVEM: Event = scout(2, {
  id: id(14),
  kind: "charge",
  title: "Nuvem Drive renews · €9.99",
  starts: "2026-10-28",
  ends: "2026-10-29",
  all_day: true,
  found_by: "model",
});
const PARCEL: Event = scout(3, {
  id: id(15),
  kind: "delivery",
  title: "Parcel from Lojamar arrives",
  starts: "2026-10-28T12:00:00+00:00",
  ends: "2026-10-28T18:00:00+00:00",
});
export const REVIEW: Event = {
  ...base,
  id: id(16),
  kind: "meeting",
  title: "Design review with Inês",
  starts: "2026-10-28T16:00:00+00:00",
  ends: "2026-10-28T17:00:00+00:00",
  origin: "provider",
  calendar_id: WORK.id,
  on_provider: true,
  response: "needs_action",
  notice: "kept_theirs",
  place: "Room 2",
};
const QUARTET: Event = scout(4, {
  id: id(17),
  kind: "show",
  title: "The Lisbon Quartet",
  starts: "2026-10-29T21:00:00+00:00",
  ends: "2026-10-29T23:00:00+00:00",
  place: "Coliseu dos Recreios",
  on_provider: true,
});
export const FLIGHT: Event = scout(5, {
  id: id(18),
  kind: "travel",
  title: "Flight LIS → AMS · AL 1672",
  starts: "2026-10-30T07:40:00+00:00",
  ends: "2026-10-30T10:25:00+00:00",
  time_zone: "Europe/Amsterdam",
  place: "Lisbon Airport T1 → Amsterdam Schiphol",
});
const HOTEL: Event = scout(6, {
  id: id(19),
  kind: "stay",
  title: "Hotel Canal Verde · 2 nights",
  starts: "2026-10-30T14:00:00+00:00",
  ends: "2026-11-01T11:00:00+00:00",
  time_zone: "Europe/Amsterdam",
  place: "Prinsengracht 400, Amsterdam",
  found_by: "model",
});
const DINNER: Event = scout(7, {
  id: id(20),
  kind: "reservation",
  title: "Dinner for 2 · Restaurante Dijk",
  starts: "2026-10-30T19:00:00+00:00",
  ends: "2026-10-30T21:00:00+00:00",
  time_zone: "Europe/Amsterdam",
  place: "Restaurante Dijk, Amsterdam",
});
const BACK: Event = scout(5, {
  id: id(21),
  kind: "travel",
  title: "Flight AMS → LIS · AL 1677",
  starts: "2026-11-01T17:05:00+00:00",
  ends: "2026-11-01T20:00:00+00:00",
  time_zone: "Europe/Amsterdam",
  place: "Amsterdam Schiphol → Lisbon Airport T1",
});
const INSURANCE: Event = scout(8, {
  id: id(22),
  kind: "expiry",
  title: "Car insurance ends",
  starts: "2026-11-02",
  ends: "2026-11-03",
  all_day: true,
  found_by: "model",
});

export const WEEK_EVENTS: Event[] = [
  STANDUP,
  SYNC,
  HOLD,
  DENTIST,
  NUVEM,
  PARCEL,
  REVIEW,
  QUARTET,
  FLIGHT,
  HOTEL,
  DINNER,
  BACK,
  INSURANCE,
];

/** Who wrote to Ana: the senders of the emails above, by message number. */
const SENDERS: Record<number, { name: string; address: string; subject: string; sent: string }> = {
  1: {
    name: "Clínica Sorriso",
    address: "agenda@clinica-sorriso.example",
    subject: "Your appointment is confirmed",
    sent: "2026-10-19T09:10:00Z",
  },
  2: {
    name: "Nuvem",
    address: "billing@nuvem.example",
    subject: "Your plan renews on 28 October",
    sent: "2026-10-21T08:00:00Z",
  },
  3: {
    name: "Lojamar",
    address: "orders@lojamar.example",
    subject: "Your order is on its way",
    sent: "2026-10-26T10:30:00Z",
  },
  4: {
    name: "Bilheteira Online",
    address: "tickets@bilheteira.example",
    subject: "Your tickets for The Lisbon Quartet",
    sent: "2026-09-02T12:00:00Z",
  },
  5: {
    name: "Aerolis",
    address: "booking@aerolis.example",
    subject: "Booking confirmed: Lisbon → Amsterdam",
    sent: "2026-10-21T07:45:00Z",
  },
  6: {
    name: "Canal Verde",
    address: "stay@canalverde.example",
    subject: "Your reservation, 30 Oct – 1 Nov",
    sent: "2026-10-21T08:20:00Z",
  },
  7: {
    name: "Mesa",
    address: "tables@mesa.example",
    subject: "Table for 2 confirmed",
    sent: "2026-10-22T18:00:00Z",
  },
  8: {
    name: "Seguros Atlântico",
    address: "avisos@seguros-atlantico.example",
    subject: "Your policy renewal notice",
    sent: "2026-10-12T09:00:00Z",
  },
};

/** The conversation of message `n` (for `conversations.get` behind an event's sheet). */
export function sourceDetail(n: number): Detail {
  const sender = SENDERS[n] ?? SENDERS[1];
  if (sender === undefined) {
    throw new Error("no such demo sender");
  }
  const summary: Summary = {
    ...Q4,
    id: conversation(n),
    subject: sender.subject,
    message_count: 1,
    unread_count: 0,
    last_message_at: sender.sent,
    latest_from: { name: sender.name, address: sender.address },
    snippet: sender.subject,
    labels: [],
    last_sent_at: null,
    sent_to: [],
    replied_since_sent: false,
    stream: "updates",
  };
  return {
    conversation: summary,
    messages: [
      {
        id: message(n),
        direction: "inbound",
        from: { name: sender.name, address: sender.address },
        to: [{ name: INBOX.display_name, address: INBOX.address }],
        cc: [],
        reply_to: [],
        subject: sender.subject,
        sent_at: sender.sent,
        read: true,
        starred: false,
        text: { content: sender.subject, truncated: false },
        has_html: false,
        attachments: [],
      },
    ],
    decisions: [],
  };
}

export const SOURCE_NUMBERS = [1, 2, 3, 4, 5, 6, 7, 8] as const;
export const conversationId = conversation;
export const messageId = message;

/** What the model read from the dentist's email, for the sentence in the sheet. */
export const DENTIST_READ: Read = {
  asks: [],
  can_read: false,
  flagged: true,
  items: [
    {
      key: `${message(1)}:model:item:0`,
      kind: "appointment",
      title: DENTIST.title,
      starts: DENTIST.starts,
      ends: DENTIST.ends,
      all_day: false,
      place: DENTIST.place ?? null,
      quote: "Your appointment is confirmed for Tuesday 27 October at 15:00.",
    },
  ],
  layers: ["model"],
  status: "done",
};

/** Tasks due, as the Due layer will draw them once `tasks` publishes them. */
export const DUE_ITEMS = [
  {
    id: id(91),
    verb: "pay",
    title: "EDP bill",
    amount: { minor: 8420, currency: "EUR" },
    due: "2026-10-25",
    source: {
      conversationId: conversation(9),
      stream: "receipts",
      subject: "Your October bill",
      sender: "EDP",
      sentAt: "2026-10-12T08:00:00Z",
      quote: "Please pay €84.20 by 25 October 2026.",
    },
  },
  {
    id: id(92),
    verb: "pay",
    title: "Costa Obras invoice",
    amount: { minor: 124_000, currency: "EUR" },
    due: "2026-10-27",
    source: {
      conversationId: conversation(10),
      stream: "primary",
      subject: "Invoice 2026-118, works at Rua da Prata",
      sender: "Rui Costa",
      sentAt: "2026-10-20T10:00:00Z",
      quote: "Please settle invoice 2026-118 (€1,240.00) by 27 October.",
    },
  },
  {
    id: id(93),
    verb: "do",
    title: "Send the signed lease",
    due: "2026-10-29",
    source: {
      conversationId: conversation(11),
      stream: "primary",
      subject: "Lease for the Alfama flat",
      sender: "Marta Lopes",
      sentAt: "2026-10-26T18:12:00Z",
      quote: "Can you send me the signed lease by Thursday 29 October?",
    },
  },
] as const;

/** The email the reader stories open: a lease with a dated handover and a deadline. */
export const LEASE: Summary = {
  ...Q4,
  id: conversation(11),
  subject: "Lease for the Alfama flat",
  stream: "primary",
  last_message_at: "2026-10-26T18:12:00Z",
  message_count: 1,
  unread_count: 1,
  latest_from: { name: "Marta Lopes", address: "marta@lopesimoveis.example" },
  snippet: "Can you send me the signed lease by Thursday 29 October?",
  labels: [],
  last_sent_at: null,
  sent_to: [],
  replied_since_sent: false,
  // The same ask as the "Do · Send the signed lease" row on the calendar.
  need: {
    reason: "do",
    trust: "sure",
    question: "asks_action",
    basis: "model",
    rule: "model.decide",
    due_on: "2026-10-29",
    overdue: false,
    urgent: false,
    message_id: message(11),
    sent_at: "2026-10-26T18:12:00Z",
    quote: {
      text: "Can you send me the signed lease by Thursday 29 October?",
      from: { name: "Marta Lopes", address: "marta@lopesimoveis.example" },
      sent_at: "2026-10-26T18:12:00Z",
    },
  },
};
export const LEASE_MESSAGE = message(11);
export const LEASE_DETAIL: Detail = {
  conversation: LEASE,
  messages: [
    {
      id: LEASE_MESSAGE,
      direction: "inbound",
      from: { name: "Marta Lopes", address: "marta@lopesimoveis.example" },
      to: [{ name: INBOX.display_name, address: INBOX.address }],
      cc: [],
      reply_to: [],
      subject: LEASE.subject,
      sent_at: LEASE.last_message_at,
      read: false,
      starred: false,
      text: {
        content:
          "Hi Ana,\n\nThanks for confirming the flat. Can you send me the signed lease by Thursday 29 October? A scan is fine for now.\n\nThe key handover is on Saturday 31 October at 10:00 at the flat. I will bring both sets and the meter readings.\n\nBest,\nMarta",
        truncated: false,
      },
      has_html: false,
      attachments: [],
    },
  ],
  decisions: [],
};

/** The handover Scout read from the lease email, and its event. */
export const HANDOVER_ITEM: Read["items"][number] = {
  key: `${LEASE_MESSAGE}:model:item:0`,
  kind: "appointment",
  title: "Key handover at the flat",
  starts: "2026-10-31T10:00:00+00:00",
  ends: "2026-10-31T10:45:00+00:00",
  all_day: false,
  place: "Rua dos Remédios 14",
  time_zone: SCHEDULE_ZONE,
  quote: "The key handover is on Saturday 31 October at 10:00 at the flat.",
};
export const HANDOVER: Event = {
  ...base,
  id: id(30),
  kind: "appointment",
  title: HANDOVER_ITEM.title,
  starts: HANDOVER_ITEM.starts,
  ends: HANDOVER_ITEM.ends ?? HANDOVER_ITEM.starts,
  place: HANDOVER_ITEM.place ?? null,
  found_by: "model",
  source: { type: "conversations.message", id: LEASE_MESSAGE },
  source_conversation: LEASE.id,
};

const noRead = {
  asks: [],
  items: [],
  layers: [],
  dated_kind: null,
} satisfies Pick<Read, "asks" | "items" | "layers" | "dated_kind">;

/** Scout has read the lease email. */
export const READ_DONE: Read = {
  ...noRead,
  asks: [],
  items: [HANDOVER_ITEM],
  layers: ["model"],
  can_read: false,
  flagged: true,
  status: "done",
};
/** Flagged, never read: opening the email asks for the read. */
export const READ_UNREAD: Read = { ...noRead, can_read: true, flagged: true, status: "none" };
/** No model may read mail on this server: the pass only knows it has an appointment. */
export const READ_FALLBACK: Read = {
  ...noRead,
  dated_kind: "appointment",
  can_read: false,
  flagged: true,
  status: "none",
};
