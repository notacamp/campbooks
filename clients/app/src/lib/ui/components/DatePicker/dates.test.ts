import { describe, expect, it } from "vitest";

import {
  addDays,
  addMonths,
  clampDay,
  endOfWeek,
  inRange,
  monthMatrix,
  parseDay,
  sameMonth,
  startOfWeek,
  todayOf,
  weekdayOf,
  weekStart,
} from "./dates";

describe("dates", () => {
  it("reads only real days", () => {
    expect(parseDay("2026-10-26")).toEqual({ year: 2026, month: 10, date: 26 });
    expect(parseDay("2026-02-30")).toBeUndefined();
    expect(parseDay("0002-10-2")).toBeUndefined();
    expect(parseDay("")).toBeUndefined();
  });

  it("adds days across months, years and a daylight-saving change", () => {
    expect(addDays("2026-10-31", 1)).toBe("2026-11-01");
    expect(addDays("2026-01-01", -1)).toBe("2025-12-31");
    // Lisbon and most of Europe change their clocks on 25 October 2026.
    expect(addDays("2026-10-24", 2)).toBe("2026-10-26");
  });

  it("adds months, landing on the last day of a shorter one", () => {
    expect(addMonths("2026-01-31", 1)).toBe("2026-02-28");
    expect(addMonths("2026-03-15", -3)).toBe("2025-12-15");
    expect(addMonths("2028-02-29", 12)).toBe("2029-02-28");
  });

  it("counts weekdays from Monday and weeks from the locale's first day", () => {
    expect(weekdayOf("2026-10-26")).toBe(1);
    expect(weekdayOf("2026-10-25")).toBe(7);
    expect(startOfWeek("2026-10-29", 1)).toBe("2026-10-26");
    expect(startOfWeek("2026-10-29", 7)).toBe("2026-10-25");
    expect(endOfWeek("2026-10-29", 1)).toBe("2026-11-01");
  });

  it("lays a month out as six weeks from the week of its first day", () => {
    const weeks = monthMatrix("2026-10-14", 1);
    expect(weeks).toHaveLength(6);
    expect(weeks.every((week) => week.length === 7)).toBe(true);
    // 1 October 2026 is a Thursday.
    expect(weeks[0]?.[0]).toBe("2026-09-28");
    expect(weeks[0]?.[3]).toBe("2026-10-01");
    expect(weeks[5]?.[6]).toBe("2026-11-08");
    expect(monthMatrix("2026-10-14", 7)[0]?.[0]).toBe("2026-09-27");
  });

  it("keeps a day inside its bounds", () => {
    expect(clampDay("2026-10-01", "2026-10-13")).toBe("2026-10-13");
    expect(clampDay("2026-12-01", undefined, "2026-11-30")).toBe("2026-11-30");
    expect(clampDay("2026-10-20", "2026-10-13", "2026-11-30")).toBe("2026-10-20");
    expect(inRange("2026-10-12", "2026-10-13")).toBe(false);
    expect(inRange("2026-10-13", "2026-10-13")).toBe(true);
    expect(sameMonth("2026-10-01", "2026-10-31")).toBe(true);
    expect(sameMonth("2026-10-31", "2026-11-01")).toBe(false);
  });

  it("reads today on the device's calendar", () => {
    expect(todayOf(new Date(2026, 9, 13, 23, 30))).toBe("2026-10-13");
  });

  it("starts the week where the locale does, Monday when it cannot tell", () => {
    expect([1, 2, 3, 4, 5, 6, 7]).toContain(weekStart("pt-PT"));
    expect(weekStart("not a locale")).toBe(1);
  });
});
