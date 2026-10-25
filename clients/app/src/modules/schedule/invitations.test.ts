import { describe, expect, it } from "vitest";

import { busyThen, invitationWindow } from "./invitations";
import { DAY, event } from "./model.fixtures";

/** An hour-long event on `DAY` from `start` ("14:00"). */
const at = (id: string, start: string, end: string, fields: Parameters<typeof event>[0] = {}) =>
  event({
    id,
    kind: "meeting",
    title: id,
    starts: `${DAY}T${start}:00+00:00`,
    ends: `${DAY}T${end}:00+00:00`,
    ...fields,
  });

const INVITE = at("offsite", "14:00", "15:00", { response: "needs_action" });

describe("Free or busy when an invitation happens", () => {
  it("is free when nothing overlaps it", () => {
    expect(busyThen(INVITE, [INVITE, at("lunch", "12:30", "14:00")])).toBeUndefined();
  });

  it("names the earliest busy event that overlaps it, held time included", () => {
    const standup = at("standup", "14:30", "14:45");
    const hold = at("lease", "13:45", "14:15", { kind: "hold" });
    expect(busyThen(INVITE, [INVITE, standup, hold])?.id).toBe("lease");
  });

  it("leaves out what does not make the member busy", () => {
    const others = [
      at("declined", "14:00", "15:00", { response: "declined" }),
      at("free", "14:00", "15:00", { busy: false }),
      at("cancelled", "14:00", "15:00", { status: "cancelled" }),
      event({ id: "holiday", all_day: true, starts: DAY, ends: "2026-10-28" }),
    ];
    expect(busyThen(INVITE, [INVITE, ...others])).toBeUndefined();
  });

  it("says nothing for an all-day invitation", () => {
    const allDay = event({ id: "fair", all_day: true, starts: DAY, ends: "2026-10-28" });
    expect(busyThen(allDay, [allDay, at("call", "10:00", "11:00")])).toBeUndefined();
  });
});

describe("The window Home reads", () => {
  it("is Home's week when every invitation is within it", () => {
    expect(invitationWindow(DAY, [{ id: "a", on: "2026-10-29" }])).toEqual({
      from: DAY,
      to: "2026-11-04",
    });
    expect(invitationWindow(DAY, [])).toEqual({ from: DAY, to: "2026-11-04" });
  });

  it("reaches the day after the last invitation, never past a year", () => {
    expect(
      invitationWindow(DAY, [
        { id: "a", on: "2026-11-08" },
        { id: "b", on: null },
      ]),
    ).toEqual({ from: DAY, to: "2026-11-09" });
    expect(invitationWindow(DAY, [{ id: "c", on: "2028-01-01" }]).to).toBe("2027-10-27");
  });
});
