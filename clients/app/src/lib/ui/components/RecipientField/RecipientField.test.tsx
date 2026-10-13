import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { isEmailAddress, parseRecipients, RecipientField } from "./RecipientField";

describe("parseRecipients", () => {
  it("reads addresses, names and lists", () => {
    expect(
      parseRecipients('Ana Lopes <ana@example.com>, leo@example.com; "Mo" <mo@example.com>'),
    ).toEqual([
      { name: "Ana Lopes", address: "ana@example.com" },
      { address: "leo@example.com" },
      { name: "Mo", address: "mo@example.com" },
    ]);
    expect(parseRecipients("<x@example.com>")).toEqual([{ name: null, address: "x@example.com" }]);
    expect(parseRecipients(" , ")).toEqual([]);
  });

  it("checks addresses", () => {
    expect(isEmailAddress("ana@example.com")).toBe(true);
    expect(isEmailAddress("ana@example")).toBe(false);
    expect(isEmailAddress("ana example.com")).toBe(false);
    expect(isEmailAddress("ana@exa mple.com")).toBe(false);
  });
});

describe("RecipientField", () => {
  it("adds typed and pasted addresses as chips, once each", () => {
    const onChange = vi.fn();
    renderUi(
      <RecipientField
        label="To"
        recipients={[{ name: "Ana", address: "ana@example.com" }]}
        onChange={onChange}
      />,
    );
    const input = screen.getByRole("textbox", { name: "To" });
    fireEvent.change(input, { target: { value: "leo@example.com" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenLastCalledWith([
      { name: "Ana", address: "ana@example.com" },
      { address: "leo@example.com" },
    ]);
    fireEvent.change(input, { target: { value: "ANA@example.com, mo@example.com" } });
    expect(onChange).toHaveBeenLastCalledWith([
      { name: "Ana", address: "ana@example.com" },
      { address: "mo@example.com" },
    ]);
  });

  it("refuses an invalid address and says why", () => {
    const onChange = vi.fn();
    renderUi(<RecipientField label="Cc" recipients={[]} onChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Cc" });
    fireEvent.change(input, { target: { value: "nope" } });
    fireEvent.keyDown(input, { key: "Tab" });
    expect(onChange).not.toHaveBeenCalled();
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByText("This is not an email address: nope")).toBeInTheDocument();
    fireEvent.change(input, { target: { value: "nope@example.com" } });
    expect(input).not.toHaveAttribute("aria-invalid");
    fireEvent.blur(input);
    expect(onChange).toHaveBeenCalledWith([{ address: "nope@example.com" }]);
  });

  it("removes chips with their button or Backspace", () => {
    const onChange = vi.fn();
    const recipients = [
      { name: "Ana", address: "ana@example.com" },
      { address: "leo@example.com" },
    ];
    renderUi(<RecipientField label="To" recipients={recipients} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Remove Ana" }));
    expect(onChange).toHaveBeenLastCalledWith([{ address: "leo@example.com" }]);
    fireEvent.keyDown(screen.getByRole("textbox", { name: "To" }), { key: "Backspace" });
    expect(onChange).toHaveBeenLastCalledWith([{ name: "Ana", address: "ana@example.com" }]);
    fireEvent.keyDown(screen.getByRole("textbox", { name: "To" }), { key: "," });
    expect(onChange).toHaveBeenCalledTimes(2);
  });
});
