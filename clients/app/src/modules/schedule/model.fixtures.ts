import type { EventView } from "./api/types";

/** Tuesday 27 October 2026. */
export const DAY = "2026-10-27";

/** An event with every field a test does not care about filled in. */
export function event(fields: Partial<EventView> = {}): EventView {
  return {
    id: "event",
    kind: "none",
    title: "Event",
    starts: "2026-10-27T09:00:00+00:00",
    ends: "2026-10-27T10:00:00+00:00",
    all_day: false,
    busy: true,
    calendar_id: "own",
    on_provider: false,
    item_keys: [],
    origin: "member",
    pending: false,
    recurring: false,
    status: "confirmed",
    ...fields,
  };
}

/** A week with a one-day all-day event and a hotel stay across three days. */
export const ANA_EVENTS: EventView[] = [
  event({ id: "nuvem", kind: "charge", all_day: true, starts: "2026-10-28", ends: "2026-10-29" }),
  event({
    id: "hotel",
    kind: "stay",
    starts: "2026-10-30T14:00:00+00:00",
    ends: "2026-11-01T11:00:00+00:00",
  }),
  event({ id: "sync", kind: "meeting" }),
];
