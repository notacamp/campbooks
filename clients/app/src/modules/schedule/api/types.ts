import type { Schema } from "~/lib/api";

export type EventView = Schema<"EventView">;
export type EventKind = Schema<"EventKind">;
export type CalendarView = Schema<"CalendarView">;
export type CalendarRole = Schema<"CalendarRole">;
export type SettingsView = Schema<"SettingsView">;
export type Response = Schema<"Response">;
export type MessageRead = Schema<"MessageReadView">;
export type ReadItem = Schema<"ReadItem">;
export type NewEvent = Schema<"NewEvent">;
export type EventPatch = Schema<"EventPatch">;
export type WorkingHours = Schema<"BTreeMap">;

/** The kinds Scout reads out of mail (a member's own events are `none`; held time is `hold`). */
export const SCOUT_KINDS = [
  "travel",
  "stay",
  "show",
  "reservation",
  "appointment",
  "meeting",
  "delivery",
  "charge",
  "expiry",
] as const satisfies readonly EventKind[];
export type ScoutKind = (typeof SCOUT_KINDS)[number];

/** Every kind a member may give an event. */
export const MEMBER_KINDS = [...SCOUT_KINDS, "none"] as const satisfies readonly EventKind[];
