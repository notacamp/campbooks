import { fireEvent, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { DEFAULT_SHORTCUTS, ShortcutRegistry } from "~/lib/shortcuts";

import { renderUi as render } from "../../../../../test/render";

import { ItemActions, type ItemAction } from "./ItemActions";

const OWN: ItemAction[] = [
  { id: "reply", label: "Reply", key: "item.choice1", ai: true },
  { id: "calendar", label: "Add to calendar", key: "item.choice2" },
];
const UNIVERSAL: ItemAction[] = [
  { id: "done", label: "Done", key: "item.done" },
  { id: "snooze", label: "Snooze…", key: "item.snooze" },
  { id: "not_needed", label: "Not needed", key: "conversations.resolve_need" },
];

// Not needed is the conversations module's key (X), as in the app.
const renderUi = (ui: Parameters<typeof render>[0]) =>
  render(ui, {
    registry: new ShortcutRegistry([
      ...DEFAULT_SHORTCUTS,
      {
        id: "conversations.resolve_need",
        keys: "x",
        scope: "item",
        group: "item",
        label: "shortcuts:item.done",
      },
    ]),
  });

function Controlled({ onAction }: { onAction: (id: string) => void }) {
  const [highlighted, setHighlighted] = useState<string | undefined>();
  return (
    <>
      <button
        type="button"
        onClick={() => {
          setHighlighted("snooze");
        }}
      >
        highlight snooze
      </button>
      <ItemActions
        label="Actions for Marta Ferreira"
        own={OWN}
        universal={UNIVERSAL}
        highlighted={highlighted}
        onHighlight={setHighlighted}
        onAction={onAction}
      />
    </>
  );
}

describe("ItemActions", () => {
  it("shows every action in one style, with its key, own actions first", () => {
    renderUi(<Controlled onAction={vi.fn()} />);
    const group = screen.getByRole("group", { name: "Actions for Marta Ferreira" });
    const buttons = [...group.querySelectorAll("button")];

    expect(buttons.map((b) => b.textContent)).toEqual([
      "Reply1",
      "Add to calendar2",
      "DoneE",
      "Snooze…S",
      "Not neededX",
    ]);
    expect(new Set(buttons.map((b) => b.dataset["variant"]))).toEqual(new Set(["secondary"]));
    expect(buttons[0]).toHaveAttribute("data-ai", "true");
    expect(buttons[2]).toHaveAttribute("aria-keyshortcuts", "E");
  });

  it("keeps one button in the tab order, and the focus follows the highlight", () => {
    renderUi(<Controlled onAction={vi.fn()} />);
    const tabbable = () =>
      screen.getAllByRole("button").filter((b) => b.closest("[role=group]") && b.tabIndex === 0);

    expect(tabbable().map((b) => b.textContent)).toEqual(["Reply1"]);
    fireEvent.click(screen.getByRole("button", { name: "highlight snooze" }));
    const snooze = screen.getByRole("button", { name: /Snooze/ });
    expect(snooze).toHaveFocus();
    expect(snooze).toHaveAttribute("data-highlighted", "true");
    expect(tabbable()).toEqual([snooze]);
  });

  it("moves the highlight to a focused button and acts on a click", () => {
    const onAction = vi.fn();
    renderUi(<Controlled onAction={onAction} />);
    const done = screen.getByRole("button", { name: /Done/ });

    fireEvent.focus(done);
    expect(done).toHaveAttribute("data-highlighted", "true");
    fireEvent.click(done);
    expect(onAction).toHaveBeenCalledWith("done");
  });

  it("draws no divider when one group is empty", () => {
    const { container } = renderUi(
      <ItemActions label="Actions" own={[]} universal={UNIVERSAL} onAction={vi.fn()} />,
    );

    expect(container.querySelector("[aria-hidden='true'][class*='divider']")).toBeNull();
  });

  it("marks own actions the assistant picked with the visor, named", () => {
    renderUi(
      <ItemActions
        label="Actions"
        own={OWN}
        universal={UNIVERSAL}
        picked="Scout picked these"
        onAction={vi.fn()}
      />,
    );

    expect(screen.getByRole("img", { name: "Scout picked these" })).toBeInTheDocument();
  });

  it("says nothing was picked when nothing was", () => {
    renderUi(<ItemActions label="Actions" own={OWN} universal={UNIVERSAL} onAction={vi.fn()} />);

    expect(screen.queryByRole("img", { name: "Scout picked these" })).toBeNull();
  });
});
