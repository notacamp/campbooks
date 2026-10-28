import { describe, expect, it } from "vitest";

import { ANA_EVENTS, DAY, event } from "./model.fixtures";
import {
  addDays,
  addMonths,
  agendaOn,
  atWallClock,
  blocksOn,
  canPush,
  takesTime,
  cityOf,
  clock,
  dayLong,
  daysBetween,
  daysOf,
  hoursFor,
  inZone,
  isYmd,
  laneOf,
  lastDay,
  minutesOf,
  nowIn,
  sameMoment,
  shift,
  spansDays,
  weekStart,
  windowOf,
  zoneOffset,
} from "./model";

describe("days", () => {
  it("adds days and months across month ends and leap years", () => {
    expect(addDays("2026-10-31", 1)).toBe("2026-11-01");
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
    expect(addDays("2028-02-28", 1)).toBe("2028-02-29");
    expect(addMonths("2026-12-15", 1)).toBe("2027-01-01");
    expect(addMonths("2026-01-31", -1)).toBe("2025-12-01");
    expect(daysBetween("2026-10-25", "2026-10-27")).toBe(2);
    expect(daysBetween("2026-10-27", "2026-10-25")).toBe(-2);
  });

  it("starts the week on Monday", () => {
    expect(weekStart(DAY)).toBe("2026-10-26");
    expect(weekStart("2026-10-26")).toBe("2026-10-26");
    expect(weekStart("2026-11-01")).toBe("2026-10-26");
  });

  it("checks what an address holds", () => {
    expect(isYmd("2026-10-27")).toBe(true);
    expect(isYmd("2026-13-40")).toBe(false);
    expect(isYmd("27 October")).toBe(false);
    expect(isYmd(null)).toBe(false);
  });
});

describe("windows", () => {
  it("shows seven days from the day looked at in the agenda", () => {
    expect(windowOf("agenda", DAY)).toEqual({ from: "2026-10-27", to: "2026-11-03" });
  });

  it("shows Monday to Sunday in the week", () => {
    expect(windowOf("week", DAY)).toEqual({ from: "2026-10-26", to: "2026-11-02" });
    expect(daysOf(windowOf("week", DAY))).toHaveLength(7);
  });

  it("shows whole weeks around the month", () => {
    const window = windowOf("month", DAY);

    expect(window).toEqual({ from: "2026-09-28", to: "2026-11-02" });
    expect(daysOf(window)).toHaveLength(35);
    expect(windowOf("month", "2026-11-10")).toEqual({ from: "2026-10-26", to: "2026-12-07" });
  });

  it("moves a week, or a month, with J and K", () => {
    expect(shift("agenda", DAY, 1)).toBe("2026-11-03");
    expect(shift("week", DAY, -1)).toBe("2026-10-20");
    expect(shift("month", DAY, 1)).toBe("2026-11-01");
    expect(shift("month", "2026-11-01", -1)).toBe("2026-10-01");
  });
});

describe("the member's clock", () => {
  it("reads the wall clock of an event time straight from its text", () => {
    expect(minutesOf("2026-11-06T07:40:00+00:00")).toBe(460);
    expect(minutesOf("2026-11-06")).toBe(0);
  });

  it("finds today and the time in the member's zone", () => {
    const at = new Date("2026-10-27T23:30:00Z");

    expect(nowIn("Europe/Lisbon", at)).toEqual({ day: "2026-10-27", minute: 23 * 60 + 30 });
    expect(nowIn("Europe/Amsterdam", at)).toEqual({ day: "2026-10-28", minute: 30 });
    expect(nowIn("Not/AZone", at).day).toBe("2026-10-27");
  });

  it("writes a wall-clock time with the zone's offset on that day", () => {
    expect(atWallClock("Europe/Lisbon", "2026-10-27", "09:30")).toBe("2026-10-27T09:30:00+00:00");
    expect(atWallClock("Europe/Lisbon", "2026-07-01", "09:30")).toBe("2026-07-01T09:30:00+01:00");
    expect(atWallClock("America/Sao_Paulo", "2026-10-27", "09:30")).toBe(
      "2026-10-27T09:30:00-03:00",
    );
    expect(zoneOffset("Not/AZone", new Date("2026-10-27T00:00:00Z"))).toBe("+00:00");
  });

  it("names a zone by its city", () => {
    expect(cityOf("Europe/Lisbon")).toBe("Lisbon");
    expect(cityOf("America/Sao_Paulo")).toBe("Sao Paulo");
  });

  it("writes times and days in the member's language", () => {
    expect(clock("en", 9 * 60 + 5)).toBe("09:05");
    expect(clock("pt-PT", 15 * 60)).toBe("15:00");
    expect(dayLong("pt-PT", "2026-11-06")).toBe("sexta-feira, 6 de novembro");
    expect(inZone("en", "2026-10-30T10:25:00+00:00", "Europe/Amsterdam")).toBe("11:25");
    expect(inZone("en", "2026-10-30T10:25:00+00:00", "Not/AZone")).toBeUndefined();
  });

  it("tells the same moment across offsets, and the same day for dates", () => {
    expect(sameMoment("2026-10-30T07:40:00+00:00", "2026-10-30T08:40:00+01:00")).toBe(true);
    expect(sameMoment("2026-10-30T07:40:00+00:00", "2026-10-30T07:40:00+01:00")).toBe(false);
    expect(sameMoment("2026-10-30", "2026-10-30T09:00:00+00:00")).toBe(true);
  });
});

describe("events on days", () => {
  it("covers the last day of an all-day event, its end being exclusive", () => {
    expect(lastDay(event({ all_day: true, starts: "2026-10-30", ends: "2026-11-02" }))).toBe(
      "2026-11-01",
    );
    expect(lastDay(event({ all_day: true, starts: "2026-10-28", ends: "2026-10-29" }))).toBe(
      "2026-10-28",
    );
  });

  it("gives an event ending at midnight to the day before", () => {
    const night = event({ starts: "2026-10-27T22:00:00+00:00", ends: "2026-10-28T00:00:00+00:00" });

    expect(lastDay(night)).toBe("2026-10-27");
  });

  it("shows an event of a day or more across days, not as a block", () => {
    expect(spansDays(event({ all_day: true, starts: "2026-10-28", ends: "2026-10-29" }))).toBe(
      true,
    );
    expect(
      spansDays(event({ starts: "2026-10-30T14:00:00+00:00", ends: "2026-11-01T11:00:00+00:00" })),
    ).toBe(true);
    expect(
      spansDays(event({ starts: "2026-10-27T22:00:00+00:00", ends: "2026-10-28T02:00:00+00:00" })),
    ).toBe(false);
  });

  it("lists a day's events all-day first, then in time order, leaving out cancelled ones", () => {
    const list = [
      event({
        id: "late",
        title: "Late",
        starts: "2026-10-27T18:00:00+00:00",
        ends: "2026-10-27T19:00:00+00:00",
      }),
      event({
        id: "all",
        title: "All day",
        all_day: true,
        starts: "2026-10-27",
        ends: "2026-10-28",
      }),
      event({
        id: "early",
        title: "Early",
        starts: "2026-10-27T08:00:00+00:00",
        ends: "2026-10-27T09:00:00+00:00",
      }),
      event({
        id: "gone",
        title: "Gone",
        status: "cancelled",
        starts: "2026-10-27T10:00:00+00:00",
        ends: "2026-10-27T11:00:00+00:00",
      }),
      event({
        id: "other",
        title: "Other day",
        starts: "2026-10-28T08:00:00+00:00",
        ends: "2026-10-28T09:00:00+00:00",
      }),
    ];

    expect(agendaOn(list, DAY).map((each) => each.id)).toEqual(["all", "early", "late"]);
  });

  it("puts events spanning days in the lane, over the columns they cover in the week", () => {
    const days = daysOf(windowOf("week", DAY));
    const lane = laneOf(ANA_EVENTS, days);

    expect(lane.map(({ event: each, from, to }) => [each.id, from, to])).toEqual([
      ["nuvem", 3, 4],
      ["hotel", 5, 8],
    ]);
  });

  it("clips a lane event to the days shown", () => {
    const days = daysOf(windowOf("week", "2026-11-03"));
    const long = event({ id: "trip", all_day: true, starts: "2026-10-30", ends: "2026-11-05" });

    expect(laneOf([long], days).map(({ from, to }) => [from, to])).toEqual([[1, 4]]);
    expect(laneOf([long], daysOf(windowOf("week", "2026-10-12")))).toEqual([]);
  });
});

describe("blocks of a day", () => {
  it("places events that overlap side by side, each in a column of the cluster", () => {
    const blocks = blocksOn(
      [
        event({ id: "a", starts: "2026-10-27T09:00:00+00:00", ends: "2026-10-27T11:00:00+00:00" }),
        event({ id: "b", starts: "2026-10-27T10:00:00+00:00", ends: "2026-10-27T10:30:00+00:00" }),
        event({ id: "c", starts: "2026-10-27T10:15:00+00:00", ends: "2026-10-27T10:45:00+00:00" }),
        event({ id: "d", starts: "2026-10-27T13:00:00+00:00", ends: "2026-10-27T14:00:00+00:00" }),
      ],
      DAY,
    );

    expect(blocks.map((block) => [block.event.id, block.column, block.columns])).toEqual([
      ["a", 0, 3],
      ["b", 1, 3],
      ["c", 2, 3],
      ["d", 0, 1],
    ]);
  });

  it("gives a short event a block it can be pressed on, and clips one crossing midnight", () => {
    const [short] = blocksOn(
      [event({ starts: "2026-10-27T09:00:00+00:00", ends: "2026-10-27T09:05:00+00:00" })],
      DAY,
    );
    const night = event({ starts: "2026-10-27T22:00:00+00:00", ends: "2026-10-28T02:00:00+00:00" });

    expect(short).toMatchObject({ start: 540, end: 570 });
    expect(blocksOn([night], DAY)[0]).toMatchObject({ start: 1320, end: 1440 });
    expect(blocksOn([night], "2026-10-28")[0]).toMatchObject({ start: 0, end: 120 });
  });

  it("draws 07:00 to 21:00, wider when an event asks for it", () => {
    expect(hoursFor([])).toEqual({ from: 7, to: 21 });
    const early = blocksOn(
      [event({ starts: "2026-10-27T05:30:00+00:00", ends: "2026-10-27T06:30:00+00:00" })],
      DAY,
    );
    const late = blocksOn(
      [event({ starts: "2026-10-27T21:00:00+00:00", ends: "2026-10-27T23:15:00+00:00" })],
      DAY,
    );

    expect(hoursFor(early)).toEqual({ from: 5, to: 21 });
    expect(hoursFor(late)).toEqual({ from: 7, to: 24 });
  });
});

describe("adding an event to a provider calendar", () => {
  const calendars = [
    { id: "own", source: "campbooks", writable: true, role: "mine" },
    { id: "work", source: "provider", writable: true, role: "mine" },
    { id: "holidays", source: "provider", writable: false, role: "off" },
  ];

  it("is offered for an event on the Campbooks calendar", () => {
    expect(canPush(event({ calendar_id: "own" }), calendars)).toBe(true);
  });

  it("is not for one already on a provider, or one a provider owns", () => {
    expect(canPush(event({ calendar_id: "own", on_provider: true }), calendars)).toBe(false);
    expect(canPush(event({ calendar_id: "work" }), calendars)).toBe(false);
  });

  it("is not offered when no provider calendar can be written", () => {
    const readOnly = calendars.filter((calendar) => calendar.id !== "work");

    expect(canPush(event({ calendar_id: "own" }), readOnly)).toBe(false);
  });

  it("is not offered when the only writable calendar is followed or off", () => {
    for (const role of ["following", "off"]) {
      const others = calendars.map((calendar) =>
        calendar.id === "work" ? { ...calendar, role } : calendar,
      );

      expect(canPush(event({ calendar_id: "own" }), others)).toBe(false);
    }
  });
});

describe("a calendar that takes the member's time", () => {
  const work = { source: "provider", writable: true, role: "mine" };

  it("is a writable provider calendar of their own", () => {
    expect(takesTime(work)).toBe(true);
    expect(takesTime({ ...work, writable: false })).toBe(false);
    expect(takesTime({ ...work, source: "campbooks" })).toBe(false);
  });

  it("is not one they follow or have turned off", () => {
    expect(takesTime({ ...work, role: "following" })).toBe(false);
    expect(takesTime({ ...work, role: "off" })).toBe(false);
  });
});
