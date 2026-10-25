import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { SuggestionMenu } from "./SuggestionMenu";

const groups = [
  {
    id: "basic",
    label: "Basic",
    items: [
      { id: "text", label: "Text", glyph: "T" },
      { id: "todo", label: "To-do", icon: "check", hint: "[]" },
    ],
  },
] as const;

describe("SuggestionMenu", () => {
  it("marks the active item and picks one on a click", () => {
    const onPick = vi.fn();
    renderUi(
      <SuggestionMenu
        label="Insert a block"
        groups={groups}
        active="todo"
        onPick={onPick}
        anchor={{ x: 10, y: 10 }}
        empty="Nothing matches."
      />,
    );
    expect(screen.getByRole("listbox", { name: "Insert a block" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /To-do/u })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: /Text/u })).toHaveAttribute("aria-selected", "false");
    fireEvent.click(screen.getByRole("option", { name: /Text/u }));
    expect(onPick).toHaveBeenCalledWith("text");
  });

  it("says so when nothing matches", () => {
    renderUi(
      <SuggestionMenu
        label="Mention"
        groups={[]}
        active={undefined}
        onPick={vi.fn()}
        anchor={{ x: 0, y: 0 }}
        empty="Nothing matches."
      />,
    );
    expect(screen.getByText("Nothing matches.")).toBeInTheDocument();
  });

  it("says that it is still looking while it is", () => {
    renderUi(
      <SuggestionMenu
        label="Mention"
        groups={[]}
        active={undefined}
        onPick={vi.fn()}
        anchor={{ x: 0, y: 0 }}
        empty="Nothing matches."
        loading="Searching…"
      />,
    );
    expect(screen.getByText("Searching…")).toBeInTheDocument();
    expect(screen.queryByText("Nothing matches.")).toBeNull();
  });
});
