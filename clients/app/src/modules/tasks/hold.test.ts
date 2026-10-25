import { describe, expect, it } from "vitest";

import { LEASE } from "../../../test/demo/tasks";

import { heldLabel, holdable, holdSlots, holdWindow, untilDue } from "./hold";

const NOW = new Date(2026, 9, 27, 14, 20);
const at = (day: number, hours: number, minutes = 0) =>
  new Date(2026, 9, day, hours, minutes).toISOString();

describe("Hold time", () => {
  it("offers the start of each free stretch first", () => {
    const slots = holdSlots(
      [
        { starts: at(28, 9), ends: at(28, 10) },
        { starts: at(28, 14), ends: at(28, 15) },
        { starts: at(29, 11), ends: at(29, 12) },
        { starts: at(30, 9), ends: at(30, 10) },
      ],
      30,
      NOW,
    );
    expect(slots.map((slot) => slot.starts)).toEqual([at(28, 9), at(28, 14), at(29, 11)]);
    expect(slots[0]?.ends).toBe(at(28, 9, 30));
  });

  it("fills three from one long afternoon, a length apart", () => {
    const slots = holdSlots([{ starts: at(28, 14), ends: at(28, 18) }], 60, NOW);
    expect(slots.map((slot) => slot.starts)).toEqual([at(28, 14), at(28, 15), at(28, 16)]);
  });

  it("starts no earlier than a moment from now, on a five-minute mark", () => {
    const [first] = holdSlots([{ starts: at(27, 14, 0), ends: at(27, 16) }], 15, NOW);
    expect(first?.starts).toBe(at(27, 14, 25));
  });

  it("offers nothing when no stretch is long enough", () => {
    expect(holdSlots([{ starts: at(28, 9), ends: at(28, 9, 20) }], 30, NOW)).toEqual([]);
  });

  it("looks until the end of the due day, or the next 7 days", () => {
    const due = { ...LEASE, due_on: "2026-10-30" };
    expect(holdWindow(due, NOW, 30)).toEqual({
      from: NOW.toISOString(),
      to: new Date(2026, 9, 31).toISOString(),
      minutes: 30,
    });
    expect(untilDue(due, NOW)).toBe(true);
    const late = { ...LEASE, due_on: "2026-10-20" };
    expect(holdWindow(late, NOW, 15).to).toBe(new Date(2026, 10, 3, 14, 20).toISOString());
    expect(untilDue(late, NOW)).toBe(false);
  });

  it("says when it is held, and is offered for open Do tasks only", () => {
    expect(heldLabel({ starts: at(28, 10), ends: at(28, 10, 30) }, "en")).toBe("Wed 10:00–10:30");
    expect(holdable({ ...LEASE, verb: "do", state: "open" })).toBe(true);
    expect(holdable({ ...LEASE, verb: "pay", state: "open" })).toBe(false);
    expect(holdable({ ...LEASE, verb: "do", state: "done" })).toBe(false);
  });
});
