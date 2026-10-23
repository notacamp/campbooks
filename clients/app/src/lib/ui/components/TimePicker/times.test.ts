import { describe, expect, it } from "vitest";

import {
  formatTime,
  fromMinutes,
  isTime,
  nearestSlot,
  parseTime,
  timeSlots,
  toMinutes,
} from "./times";

describe("times", () => {
  it("reads the ways a member types a time", () => {
    expect(parseTime("9")).toBe("09:00");
    expect(parseTime("930")).toBe("09:30");
    expect(parseTime("0930")).toBe("09:30");
    expect(parseTime("9:30")).toBe("09:30");
    expect(parseTime("21:15")).toBe("21:15");
    expect(parseTime("9.30")).toBe("09:30");
    expect(parseTime("21h15")).toBe("21:15");
    expect(parseTime(" 9pm ")).toBe("21:00");
    expect(parseTime("9:30 am")).toBe("09:30");
    expect(parseTime("9:30 P.M.")).toBe("21:30");
    expect(parseTime("12am")).toBe("00:00");
    expect(parseTime("12 pm")).toBe("12:00");
    // As English writes it, with the narrow no-break space before PM.
    expect(parseTime("9:30\u202FPM")).toBe("21:30");
  });

  it("reads nothing from what is no time", () => {
    for (const typed of ["", "noon", "25", "9:75", "13pm", "0am", "12345", "9:3"]) {
      expect(parseTime(typed), typed).toBeUndefined();
    }
  });

  it("lists the day's times every step, from the earliest one allowed", () => {
    const quarters = timeSlots(15);
    expect(quarters).toHaveLength(96);
    expect(quarters[0]).toBe("00:00");
    expect(quarters.at(-1)).toBe("23:45");
    const afterStart = timeSlots(30, "09:10");
    expect(afterStart[0]).toBe("09:30");
    expect(afterStart).not.toContain("09:00");
  });

  it("finds the slot nearest a time", () => {
    const slots = timeSlots(15);
    expect(slots[nearestSlot(slots, "09:07")]).toBe("09:00");
    expect(slots[nearestSlot(slots, "09:08")]).toBe("09:15");
    expect(nearestSlot([], "09:00")).toBe(-1);
  });

  it("counts minutes and checks whole times", () => {
    expect(toMinutes("01:30")).toBe(90);
    expect(fromMinutes(1395)).toBe("23:15");
    expect(isTime("23:59")).toBe(true);
    expect(isTime("24:00")).toBe(false);
    expect(isTime("9:30")).toBe(false);
  });

  it("writes the time as the member's language does", () => {
    expect(formatTime("21:30", "pt-PT")).toBe("21:30");
    expect(formatTime("21:30", "en")).toMatch(/^9:30\sPM$/u);
  });
});
