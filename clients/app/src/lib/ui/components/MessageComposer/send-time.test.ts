import { describe, expect, it } from "vitest";

import { fromPicked, nextWorkingMorning, sendTimeProblem, tomorrowMorning } from "./send-time";

// Local times: the helpers work in the device's time zone.
const at = (y: number, m: number, d: number, h = 15, min = 0) => new Date(y, m - 1, d, h, min);

describe("send times", () => {
  it("tomorrow is the next day at 08:00", () => {
    expect(tomorrowMorning(at(2026, 10, 28))).toEqual(at(2026, 10, 29, 8));
    // The last day of a month rolls into the next.
    expect(tomorrowMorning(at(2026, 10, 31))).toEqual(at(2026, 11, 1, 8));
  });

  it("the next working day skips the weekend", () => {
    // Wednesday 28 Oct 2026 → Thursday; Friday, Saturday, Sunday → Monday 2 Nov.
    expect(nextWorkingMorning(at(2026, 10, 28))).toEqual(at(2026, 10, 29, 8));
    expect(nextWorkingMorning(at(2026, 10, 30))).toEqual(at(2026, 11, 2, 8));
    expect(nextWorkingMorning(at(2026, 10, 31))).toEqual(at(2026, 11, 2, 8));
    expect(nextWorkingMorning(at(2026, 11, 1))).toEqual(at(2026, 11, 2, 8));
  });

  it("keeps 08:00 local across a daylight-saving change", () => {
    // Europe moves its clocks back on 25 Oct 2026: still 08:00 on the 26th.
    const monday = tomorrowMorning(at(2026, 10, 25, 9));
    expect([monday.getDate(), monday.getHours(), monday.getMinutes()]).toEqual([26, 8, 0]);
  });

  it("refuses a time too soon, in the past or more than 366 days ahead", () => {
    const now = at(2026, 10, 28, 15, 0);
    expect(sendTimeProblem(at(2026, 10, 28, 15, 1), now)).toBe("past");
    expect(sendTimeProblem(at(2026, 10, 27), now)).toBe("past");
    expect(sendTimeProblem(fromPicked("", "08:00"), now)).toBe("past");
    expect(sendTimeProblem(at(2027, 10, 30), now)).toBe("tooFar");
    expect(sendTimeProblem(fromPicked("2026-10-29", "08:00"), now)).toBeUndefined();
  });
});
