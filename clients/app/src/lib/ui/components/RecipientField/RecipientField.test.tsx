import { fireEvent, screen, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import {
  isEmailAddress,
  parseRecipients,
  RecipientField,
  type Recipient,
  type RecipientSuggestion,
} from "./RecipientField";

const PEOPLE: RecipientSuggestion[] = [
  { name: "Marta Ferreira", address: "marta@ferreira-studio.example" },
  { name: "Mário Lopes", address: "mario@lopes-arq.example" },
  { address: "mar@harbour-books.example" },
];

/** A field suggesting {@link PEOPLE} for anything two characters long or more. */
function Suggesting({
  initial = [],
  onChange,
  loading = false,
  people = PEOPLE,
}: {
  initial?: Recipient[];
  onChange?: (recipients: Recipient[]) => void;
  loading?: boolean;
  people?: RecipientSuggestion[];
}) {
  const [recipients, setRecipients] = useState<Recipient[]>(initial);
  const [query, setQuery] = useState("");
  return (
    <RecipientField
      label="To"
      recipients={recipients}
      onChange={(next) => {
        setRecipients(next);
        onChange?.(next);
      }}
      suggestions={query.trim().length >= 2 ? people : []}
      onQueryChange={setQuery}
      loadingSuggestions={loading}
    />
  );
}

/** The options listed, by their text. */
function options(): string[] {
  const list = screen.queryByRole("listbox", { name: "Suggestions for To" });
  return list === null
    ? []
    : within(list)
        .getAllByRole("option")
        .map((option) => option.textContent);
}

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

describe("RecipientField with suggestions", () => {
  it("suggests people as you type, the first highlighted", () => {
    renderUi(<Suggesting />);
    const input = screen.getByRole("combobox", { name: "To" });
    expect(input).toHaveAttribute("aria-expanded", "false");
    fireEvent.change(input, { target: { value: "m" } });
    expect(options()).toEqual([]);
    fireEvent.change(input, { target: { value: "mar" } });
    expect(input).toHaveAttribute("aria-expanded", "true");
    expect(input).toHaveAttribute("aria-autocomplete", "list");
    expect(options()).toEqual([
      "Marta Ferreiramarta@ferreira-studio.example",
      "Mário Lopesmario@lopes-arq.example",
      "mar@harbour-books.example",
    ]);
    const [first] = screen.getAllByRole("option");
    expect(first).toHaveAttribute("aria-selected", "true");
    expect(input).toHaveAttribute("aria-activedescendant", first?.id);
    expect(input).toHaveAttribute("aria-controls", screen.getByRole("listbox").id);
  });

  it("moves with the arrows, wrapping, and adds the highlighted person with Enter or Tab", () => {
    const onChange = vi.fn();
    renderUi(<Suggesting onChange={onChange} />);
    const input = screen.getByRole("combobox", { name: "To" });
    fireEvent.change(input, { target: { value: "mar" } });
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(screen.getAllByRole("option")[2]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getAllByRole("option")[1]).toHaveAttribute("aria-selected", "true");
    expect(fireEvent.keyDown(input, { key: "Enter" })).toBe(false);
    expect(onChange).toHaveBeenLastCalledWith([
      { name: "Mário Lopes", address: "mario@lopes-arq.example" },
    ]);
    expect(input).toHaveValue("");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByText("Mário Lopes")).toBeVisible();

    fireEvent.change(input, { target: { value: "mar" } });
    fireEvent.keyDown(input, { key: "Tab" });
    expect(onChange).toHaveBeenLastCalledWith([
      { name: "Mário Lopes", address: "mario@lopes-arq.example" },
      { name: "Marta Ferreira", address: "marta@ferreira-studio.example" },
    ]);
  });

  it("adds the person clicked or tapped", () => {
    const onChange = vi.fn();
    renderUi(<Suggesting onChange={onChange} />);
    const input = screen.getByRole("combobox", { name: "To" });
    fireEvent.change(input, { target: { value: "harbour" } });
    const option = screen.getByRole("option", { name: /mar@harbour-books\.example/u });
    // Before the input's blur: the pick must not wait for a click.
    expect(fireEvent.mouseDown(option)).toBe(false);
    expect(onChange).toHaveBeenLastCalledWith([
      { name: null, address: "mar@harbour-books.example" },
    ]);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("leaves out people already added", () => {
    renderUi(
      <Suggesting initial={[{ name: "Marta", address: "MARTA@ferreira-studio.example" }]} />,
    );
    const input = screen.getByRole("combobox", { name: "To" });
    fireEvent.change(input, { target: { value: "mar" } });
    expect(options()).toEqual(["Mário Lopesmario@lopes-arq.example", "mar@harbour-books.example"]);
  });

  it("shows at most eight", () => {
    const many = Array.from({ length: 10 }, (_, index) => ({
      address: `person${String(index)}@example.com`,
    }));
    renderUi(<Suggesting people={many} />);
    fireEvent.change(screen.getByRole("combobox", { name: "To" }), {
      target: { value: "person" },
    });
    expect(options()).toHaveLength(8);
  });

  it("closes on Esc, keeping Esc from the composer only while it is open", () => {
    renderUi(<Suggesting />);
    const input = screen.getByRole("combobox", { name: "To" });
    fireEvent.change(input, { target: { value: "mar" } });
    expect(fireEvent.keyDown(input, { key: "Escape" })).toBe(false);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(input).toHaveAttribute("aria-expanded", "false");
    expect(input).toHaveValue("mar");
    expect(fireEvent.keyDown(input, { key: "Escape" })).toBe(true);
    // ↓ brings it back.
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(options()).toHaveLength(3);
  });

  it("closes when the field is left", () => {
    renderUi(<Suggesting />);
    const input = screen.getByRole("combobox", { name: "To" });
    fireEvent.change(input, { target: { value: "mar" } });
    fireEvent.blur(input);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("adds what was typed once the list is closed", () => {
    const onChange = vi.fn();
    renderUi(<Suggesting onChange={onChange} />);
    const input = screen.getByRole("combobox", { name: "To" });
    fireEvent.change(input, { target: { value: "new@studio.example" } });
    expect(options()).toHaveLength(3);
    fireEvent.keyDown(input, { key: "Escape" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenLastCalledWith([{ address: "new@studio.example" }]);
    // A comma adds what was typed, list or not.
    fireEvent.change(input, { target: { value: "leo@example.com," } });
    expect(onChange).toHaveBeenLastCalledWith([
      { address: "new@studio.example" },
      { address: "leo@example.com" },
    ]);
  });

  it("says it is searching until the first people arrive", () => {
    renderUi(<Suggesting loading people={[]} />);
    const input = screen.getByRole("combobox", { name: "To" });
    fireEvent.change(input, { target: { value: "mar" } });
    expect(screen.getByRole("status")).toHaveTextContent("Searching…");
    expect(input).toHaveAttribute("aria-expanded", "false");
  });
});
