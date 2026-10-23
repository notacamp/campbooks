import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { TimePicker } from "./TimePicker";
import { formatTime } from "./times";

const en = (time: string) => formatTime(time, "en");

describe("TimePicker", () => {
  it("shows the time in the member's language and lists times on focus", () => {
    renderUi(<TimePicker label="Starts" value="09:30" onChange={vi.fn()} />);
    const field = screen.getByRole("combobox", { name: "Starts" });
    expect(field).toHaveValue(en("09:30"));
    expect(field).toHaveAttribute("aria-expanded", "false");

    fireEvent.focus(field);

    expect(field).toHaveAttribute("aria-expanded", "true");
    const list = screen.getByRole("listbox", { name: "Starts" });
    expect(within(list).getAllByRole("option")).toHaveLength(96);
    // The field's time is the active one.
    const active = screen.getByRole("option", { selected: true });
    expect(active).toHaveTextContent(en("09:30"));
    expect(field).toHaveAttribute("aria-activedescendant", active.id);
  });

  it("moves with the arrows and takes the time on Enter", () => {
    const onChange = vi.fn();
    renderUi(<TimePicker label="Starts" value="09:30" onChange={onChange} />);
    const field = screen.getByRole("combobox", { name: "Starts" });
    fireEvent.focus(field);
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(screen.getByRole("option", { selected: true })).toHaveTextContent(en("10:00"));
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.keyDown(field, { key: "Enter" });

    expect(onChange).toHaveBeenCalledWith("10:00");
    expect(field).toHaveAttribute("aria-expanded", "false");
  });

  it("picks a time with a click", () => {
    const onChange = vi.fn();
    renderUi(<TimePicker label="Ends" value={undefined} onChange={onChange} />);
    fireEvent.focus(screen.getByRole("combobox", { name: "Ends" }));
    fireEvent.mouseDown(screen.getByRole("option", { name: en("14:15") }));
    expect(onChange).toHaveBeenCalledWith("14:15");
  });

  it("takes a typed time on leaving, and goes back on anything else", () => {
    const onChange = vi.fn();
    renderUi(<TimePicker label="Starts" value="09:30" onChange={onChange} />);
    const field = screen.getByRole("combobox", { name: "Starts" });
    fireEvent.focus(field);
    fireEvent.change(field, { target: { value: "2115" } });
    // Typing jumps the list to the nearest time.
    expect(screen.getByRole("option", { selected: true })).toHaveTextContent(en("21:15"));
    fireEvent.blur(field);
    expect(onChange).toHaveBeenCalledWith("21:15");

    onChange.mockClear();
    fireEvent.focus(field);
    fireEvent.change(field, { target: { value: "half past" } });
    fireEvent.blur(field);
    expect(onChange).not.toHaveBeenCalled();
    expect(field).toHaveValue(en("09:30"));
  });

  it("closes the list on Escape, and the Escape stops there", () => {
    // What the app's shortcuts listen on: Escape there would close the dialog around.
    const onEscape = vi.fn();
    document.addEventListener("keydown", onEscape);
    renderUi(<TimePicker label="Starts" value="09:30" onChange={vi.fn()} />);
    const field = screen.getByRole("combobox", { name: "Starts" });
    fireEvent.focus(field);
    fireEvent.keyDown(field, { key: "Escape" });
    document.removeEventListener("keydown", onEscape);

    expect(field).toHaveAttribute("aria-expanded", "false");
    expect(onEscape).not.toHaveBeenCalled();
  });

  it("lists nothing before min, and refuses an earlier typed time", () => {
    const onChange = vi.fn();
    renderUi(<TimePicker label="Ends" value="10:00" min="09:30" onChange={onChange} />);
    const field = screen.getByRole("combobox", { name: "Ends" });
    fireEvent.focus(field);
    expect(screen.queryByRole("option", { name: en("09:15") })).toBeNull();
    expect(screen.getByRole("option", { name: en("09:30") })).toBeInTheDocument();

    fireEvent.change(field, { target: { value: "8:00" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onChange).not.toHaveBeenCalled();
    expect(field).toHaveValue(en("10:00"));
  });

  it("writes 24-hour times in Portuguese", () => {
    renderUi(<TimePicker label="Início" value="21:30" onChange={vi.fn()} />, { locale: "pt-PT" });
    expect(screen.getByRole("combobox", { name: "Início" })).toHaveValue("21:30");
  });
});
