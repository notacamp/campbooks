import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { DatePicker } from "./DatePicker";

const NOW = new Date(2026, 9, 13, 10);

/** A day's full name, as the grid reads it out ("Monday, October 26, 2026"). */
const named = (day: string) =>
  new Intl.DateTimeFormat("en", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(`${day}T12:00:00Z`));

const dayButton = (day: string) => screen.getByRole("button", { name: named(day) });

describe("DatePicker", () => {
  it("shows the day in words and opens on it", () => {
    renderUi(<DatePicker label="Due" value="2026-10-26" onChange={vi.fn()} now={NOW} />);
    const field = screen.getByRole("button", { name: /^Due/u });
    const short = new Intl.DateTimeFormat("en", {
      weekday: "short",
      day: "numeric",
      month: "short",
      timeZone: "UTC",
    }).format(new Date("2026-10-26T12:00:00Z"));
    expect(field).toHaveTextContent(short);
    expect(field).toHaveAttribute("aria-haspopup", "dialog");

    fireEvent.click(field);

    expect(screen.getByRole("dialog", { name: "Choose a date" })).toBeVisible();
    const grid = screen.getByRole("grid", { name: "October 2026" });
    expect(within(grid).getAllByRole("row")).toHaveLength(7);
    expect(dayButton("2026-10-26")).toHaveFocus();
    expect(dayButton("2026-10-26").closest("td")).toHaveAttribute("aria-selected", "true");
    // Today is marked, and only today.
    expect(dayButton("2026-10-13")).toHaveAttribute("aria-current", "date");
    expect(dayButton("2026-10-14")).not.toHaveAttribute("aria-current");
  });

  it("moves with the keys without picking, and picks once on Enter or a click", () => {
    const onChange = vi.fn();
    renderUi(<DatePicker label="Due" value="2026-10-26" onChange={onChange} now={NOW} />);
    const field = screen.getByRole("button", { name: /^Due/u });
    fireEvent.click(field);

    fireEvent.keyDown(dayButton("2026-10-26"), { key: "ArrowRight" });
    expect(dayButton("2026-10-27")).toHaveFocus();
    fireEvent.keyDown(dayButton("2026-10-27"), { key: "ArrowDown" });
    expect(dayButton("2026-11-03")).toHaveFocus();
    expect(screen.getByRole("grid", { name: "November 2026" })).toBeVisible();
    fireEvent.keyDown(dayButton("2026-11-03"), { key: "PageUp" });
    expect(dayButton("2026-10-03")).toHaveFocus();
    fireEvent.keyDown(dayButton("2026-10-03"), { key: "PageDown", shiftKey: true });
    expect(dayButton("2027-10-03")).toHaveFocus();
    // Only the day the keys are on can be tabbed to.
    expect(dayButton("2027-10-03")).toHaveAttribute("tabindex", "0");
    expect(dayButton("2027-10-04")).toHaveAttribute("tabindex", "-1");
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.click(dayButton("2027-10-04"));

    expect(onChange).toHaveBeenCalledOnce();
    expect(onChange).toHaveBeenCalledWith("2027-10-04");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(field).toHaveFocus();
  });

  it("closes on Escape and gives the focus back to the field", () => {
    const onChange = vi.fn();
    renderUi(<DatePicker label="Due" value={undefined} onChange={onChange} now={NOW} />);
    const field = screen.getByRole("button", { name: /^Due/u });
    expect(field).toHaveTextContent("Choose a date");
    fireEvent.click(field);
    // With no day, it opens on today.
    expect(dayButton("2026-10-13")).toHaveFocus();

    fireEvent.keyDown(dayButton("2026-10-13"), { key: "Escape" });

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(field).toHaveFocus();
    expect(onChange).not.toHaveBeenCalled();
  });

  it("keeps days outside min and max out of reach", () => {
    renderUi(
      <DatePicker
        label="Day"
        value={undefined}
        min="2026-10-13"
        max="2026-11-10"
        onChange={vi.fn()}
        now={NOW}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /^Day/u }));

    expect(dayButton("2026-10-12")).toBeDisabled();
    expect(dayButton("2026-10-13")).toBeEnabled();
    expect(screen.getByRole("button", { name: "Previous month" })).toBeDisabled();
    // The keys stop at the bounds.
    fireEvent.keyDown(dayButton("2026-10-13"), { key: "ArrowLeft" });
    expect(dayButton("2026-10-13")).toHaveFocus();
    fireEvent.click(screen.getByRole("button", { name: "Next month" }));
    expect(screen.getByRole("grid", { name: "November 2026" })).toBeVisible();
    expect(dayButton("2026-11-11")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next month" })).toBeDisabled();
  });

  it("picks today, or clears the day when it may", () => {
    const onChange = vi.fn();
    renderUi(
      <DatePicker label="Expires" value="2026-12-01" clearable onChange={onChange} now={NOW} />,
    );
    fireEvent.click(screen.getByRole("button", { name: /^Expires/u }));
    fireEvent.click(screen.getByRole("button", { name: "Today" }));
    expect(onChange).toHaveBeenLastCalledWith("2026-10-13");

    fireEvent.click(screen.getByRole("button", { name: /^Expires/u }));
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });

  it("shows the grid in place when inline", () => {
    const onChange = vi.fn();
    renderUi(
      <DatePicker label="Pick a date" value={undefined} inline onChange={onChange} now={NOW} />,
    );

    const group = screen.getByRole("group", { name: "Pick a date" });
    expect(within(group).getByRole("grid", { name: "October 2026" })).toBeVisible();
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(dayButton("2026-10-20"));
    expect(onChange).toHaveBeenCalledWith("2026-10-20");
  });

  it("says what is wrong with the day", () => {
    renderUi(
      <DatePicker
        label="Expires"
        value="2026-10-01"
        error="Pick a day in the future."
        onChange={vi.fn()}
        now={NOW}
      />,
    );
    const field = screen.getByRole("button", { name: /^Expires/u });
    expect(field).toHaveAccessibleDescription("Pick a day in the future.");
  });
});
