import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { NoteField } from "./NoteField";

function field(props: Partial<Parameters<typeof NoteField>[0]> = {}) {
  const handlers = { onChange: vi.fn(), onSubmit: vi.fn(), onCancel: vi.fn() };
  renderUi(
    <div data-testid="around">
      <NoteField
        label="About Marta, and how to write to them"
        placeholder="e.g. My landlord. Formal, in Portuguese."
        value=""
        maxLength={500}
        counter={(left) => `${String(left)} characters left`}
        {...handlers}
        {...props}
      />
    </div>,
  );
  return handlers;
}

describe("NoteField", () => {
  it("is a labelled text area that takes the focus, its text plain", () => {
    const handlers = field({ value: "<b>My landlord</b>" });
    const text = screen.getByRole("textbox", { name: "About Marta, and how to write to them" });
    expect(text).toHaveFocus();
    expect(text).toHaveValue("<b>My landlord</b>");
    expect(text).toHaveAttribute("maxlength", "500");
    expect(document.querySelector("b")).toBeNull();
    fireEvent.change(text, { target: { value: "My landlord." } });
    expect(handlers.onChange).toHaveBeenCalledWith("My landlord.");
    // Far from the limit: no counter.
    expect(screen.queryByText(/characters left/u)).toBeNull();
  });

  it("counts what is left near the limit", () => {
    field({ value: "a".repeat(470) });
    const text = screen.getByRole("textbox");
    expect(screen.getByText("30 characters left")).toBeInTheDocument();
    expect(text).toHaveAccessibleDescription("30 characters left");
  });

  it("keeps it with ⌘↵ or Ctrl+Enter and leaves with Esc, the keys stopping at the field", () => {
    const handlers = field({ value: "My landlord." });
    const outside = vi.fn();
    const around = screen.getByTestId("around");
    around.addEventListener("keydown", outside);
    const text = screen.getByRole("textbox");
    fireEvent.keyDown(text, { key: "Enter", metaKey: true });
    fireEvent.keyDown(text, { key: "Enter", ctrlKey: true });
    expect(handlers.onSubmit).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(text, { key: "Escape" });
    expect(handlers.onCancel).toHaveBeenCalledTimes(1);
    // A plain Enter is a new line, and reaches nothing.
    fireEvent.keyDown(text, { key: "Enter" });
    expect(handlers.onSubmit).toHaveBeenCalledTimes(2);
    expect(outside).toHaveBeenCalledTimes(1);
  });

  it("is read-only while kept and says what went wrong", () => {
    field({ value: "My landlord.", disabled: true, error: "Scout couldn't keep this note." });
    const text = screen.getByRole("textbox");
    expect(text).toHaveAttribute("readonly");
    expect(text).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("alert")).toHaveTextContent("Scout couldn't keep this note.");
  });
});
