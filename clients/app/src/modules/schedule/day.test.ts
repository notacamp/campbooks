import { describe, expect, it } from "vitest";

import { LEASE } from "../../../test/demo/tasks";
import type { TaskView } from "~/modules/tasks";

import type { CalendarView } from "./api/types";
import {
  COMING_MAX,
  comingUp,
  dayFacts,
  joinUrl,
  laterToday,
  NEXT_UP_AFTER,
  NEXT_UP_AHEAD,
  nextUp,
  offersAdd,
  pushTarget,
  type WallClock,
} from "./day";
import { taskRows } from "./due";
import { DAY, event } from "./model.fixtures";

/** Tuesday 27 October 2026, 10:00 on the member's clock. */
const TEN: WallClock = { day: DAY, minute: 600 };

/** A timed event starting `minutes` after 10:00 today, half an hour long. */
const at = (id: string, minutes: number, fields: Parameters<typeof event>[0] = {}) => {
  const start = 600 + minutes;
  const time = (m: number) =>
    `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
  return event({
    id,
    kind: "meeting",
    title: id,
    starts: `${DAY}T${time(start)}:00+00:00`,
    ends: `${DAY}T${time(start + 30)}:00+00:00`,
    ...fields,
  });
};

const ids = (events: readonly { id: string }[]) => events.map((each) => each.id);

describe("Next up", () => {
  it("is the earliest event starting within two hours", () => {
    expect(nextUp([at("later", 90), at("soon", 20)], TEN)?.id).toBe("soon");
  });

  it("is nothing when the next event starts in 121 minutes", () => {
    expect(NEXT_UP_AHEAD).toBe(120);
    expect(nextUp([at("far", 121)], TEN)).toBeUndefined();
    expect(nextUp([at("edge", 120)], TEN)?.id).toBe("edge");
  });

  it("stays for ten minutes once the event started", () => {
    expect(NEXT_UP_AFTER).toBe(10);
    expect(nextUp([at("started", -5)], TEN)?.id).toBe("started");
    expect(nextUp([at("gone", -10)], TEN)).toBeUndefined();
  });

  it("leaves out all-day events, held time, declined and cancelled ones", () => {
    const events = [
      event({ id: "all-day", all_day: true, starts: DAY, ends: "2026-10-28" }),
      at("held", 10, { kind: "hold" }),
      at("declined", 15, { response: "declined" }),
      at("cancelled", 20, { status: "cancelled" }),
      at("tentative", 30, { response: "tentative" }),
    ];
    expect(nextUp(events, TEN)?.id).toBe("tentative");
  });

  it("looks past midnight", () => {
    const late: WallClock = { day: DAY, minute: 23 * 60 + 30 };
    const early = event({
      id: "early",
      starts: "2026-10-28T00:15:00+00:00",
      ends: "2026-10-28T00:45:00+00:00",
    });
    expect(nextUp([early], late)?.id).toBe("early");
  });
});

describe("the rest of today", () => {
  it("lists what starts from now, all-day first, then by time, leaving Next up out", () => {
    const next = at("next", 20);
    const events = [
      at("evening", 480),
      next,
      at("held", 300, { kind: "hold" }),
      at("past", -60),
      event({ id: "holiday", all_day: true, starts: "2026-10-26", ends: "2026-10-28" }),
      at("declined", 120, { response: "declined" }),
      event({
        id: "tomorrow",
        starts: "2026-10-28T09:00:00+00:00",
        ends: "2026-10-28T10:00:00+00:00",
      }),
    ];
    expect(ids(laterToday(events, TEN, next))).toEqual(["holiday", "held", "evening"]);
  });
});

describe("Coming up", () => {
  const task = (over: Partial<TaskView>): TaskView => ({
    ...LEASE,
    due_on: null,
    due_at: null,
    plan: "none",
    plan_on: null,
    ...over,
  });

  it("holds tomorrow to a week after it, soonest first, events and due tasks", () => {
    const events = [
      event({ id: "today", starts: `${DAY}T18:00:00+00:00`, ends: `${DAY}T19:00:00+00:00` }),
      event({
        id: "thu-train",
        kind: "travel",
        starts: "2026-10-29T08:15:00+00:00",
        ends: "2026-10-29T11:00:00+00:00",
      }),
      event({
        id: "wed-charge",
        kind: "charge",
        all_day: true,
        starts: "2026-10-28",
        ends: "2026-10-29",
      }),
      event({
        id: "next-tue",
        starts: "2026-11-03T09:00:00+00:00",
        ends: "2026-11-03T10:00:00+00:00",
      }),
      event({
        id: "too-far",
        starts: "2026-11-04T09:00:00+00:00",
        ends: "2026-11-04T10:00:00+00:00",
      }),
      event({
        id: "declined",
        starts: "2026-10-30T09:00:00+00:00",
        ends: "2026-10-30T10:00:00+00:00",
        response: "declined",
      }),
    ];
    const rows = taskRows(
      [
        task({ id: "due-thu", title: "Due Thursday", due_on: "2026-10-29" }),
        task({
          id: "planned-wed",
          title: "Planned Wednesday",
          plan: "date",
          plan_on: "2026-10-28",
        }),
        task({ id: "late", title: "Late", due_on: "2026-10-20" }),
      ],
      DAY,
      "UTC",
    );
    const lines = comingUp(events, rows, TEN).map((line) =>
      line.type === "event" ? line.event.id : line.row.task.id,
    );
    expect(lines).toEqual(["wed-charge", "due-thu", "thu-train", "next-tue"]);
  });

  it("shows at most six, the soonest", () => {
    expect(COMING_MAX).toBe(6);
    const events = Array.from({ length: 8 }, (_, index) =>
      event({
        id: `e${String(index)}`,
        starts: `2026-10-28T${String(8 + index).padStart(2, "0")}:00:00+00:00`,
        ends: `2026-10-28T${String(9 + index).padStart(2, "0")}:00:00+00:00`,
      }),
    );
    expect(
      comingUp(events, [], TEN).map((line) => (line.type === "event" ? line.event.id : "")),
    ).toEqual(["e0", "e1", "e2", "e3", "e4", "e5"]);
  });
});

describe("Add to my calendar", () => {
  const own: CalendarView = {
    id: "own",
    source: "campbooks",
    name: "Campbooks",
    syncing: true,
    writable: true,
    primary: false,
  };
  const work: CalendarView = {
    id: "work",
    source: "provider",
    name: "Work",
    syncing: true,
    writable: true,
    primary: true,
  };
  const family: CalendarView = { ...work, id: "family", name: "Family", primary: false };
  const shared: CalendarView = { ...work, id: "shared", name: "Shared", writable: false };

  it("is offered for what only the member sees, when a calendar of theirs can take it", () => {
    const delivery = event({ kind: "delivery", calendar_id: "own", on_provider: false });
    expect(offersAdd(delivery, [own, work])).toBe(true);
    expect(offersAdd({ ...delivery, on_provider: true }, [own, work])).toBe(false);
    expect(offersAdd(delivery, [own, shared])).toBe(false);
    expect(offersAdd({ ...delivery, kind: "hold" }, [own, work])).toBe(false);
  });

  it("goes to the calendar added to last, else the primary one", () => {
    expect(pushTarget([own, work, family], "family")?.id).toBe("family");
    expect(pushTarget([own, family, work], null)?.id).toBe("work");
    expect(pushTarget([own, shared], null)).toBeUndefined();
  });
});

describe("Join", () => {
  it("needs a place that is an https address", () => {
    expect(joinUrl(event({ place: " https://meet.example.com/abc " }))?.host).toBe(
      "meet.example.com",
    );
    expect(joinUrl(event({ place: "http://meet.example.com/abc" }))).toBeUndefined();
    expect(joinUrl(event({ place: "Rua das Flores 12" }))).toBeUndefined();
    expect(joinUrl(event({ place: "https://" }))).toBeUndefined();
    expect(joinUrl(event({ place: null }))).toBeUndefined();
  });
});

describe("the hero's facts", () => {
  it("names the next timed event and counts the rest", () => {
    const rows = [
      event({ id: "holiday", all_day: true, starts: DAY, ends: "2026-10-28" }),
      at("held", 10, { kind: "hold" }),
      at("call", 20),
      at("lunch", 150),
    ];
    const facts = dayFacts(rows);
    expect(facts?.next.id).toBe("call");
    expect(facts?.more).toBe(3);
  });

  it("says nothing without a timed event", () => {
    expect(dayFacts([at("held", 10, { kind: "hold" })])).toBeUndefined();
    expect(dayFacts([])).toBeUndefined();
  });
});
