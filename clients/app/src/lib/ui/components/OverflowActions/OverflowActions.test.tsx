import { fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { OverflowActions, type OverflowAction } from "./OverflowActions";

/** A ResizeObserver that reports at once, the row `scrollWidth` wide in `clientWidth`. */
function measuring(scrollWidth: number, clientWidth: number) {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      callback: () => void;
      constructor(callback: () => void) {
        this.callback = callback;
      }
      observe() {
        this.callback();
      }
      disconnect() {
        // Nothing to let go of.
      }
    },
  );
  vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(scrollWidth);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(clientWidth);
}

function actions() {
  const trash = vi.fn();
  const next = vi.fn();
  const list: OverflowAction[] = [
    {
      id: "trash",
      label: "Delete",
      icon: "trash",
      shortcut: "conversations.trash",
      onSelect: trash,
    },
    { id: "pin", label: "Pin", icon: "pin", pressed: true, onSelect: vi.fn() },
  ];
  const end: OverflowAction[] = [
    { id: "next", label: "Next", icon: "chevron-down", shortcut: "list.next", onSelect: next },
  ];
  return { list, end, trash, next };
}

describe("OverflowActions", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("shows icons named by their tooltips while they fit", () => {
    measuring(200, 400);
    const { list, end, trash } = actions();
    renderUi(<OverflowActions label="More actions" actions={list} end={end} />);
    const icon = screen.getByRole("button", { name: "Delete" });
    expect(icon).toHaveAttribute("title", expect.stringContaining("Delete"));
    expect(screen.getByRole("button", { name: "Pin" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByRole("button", { name: "More actions" })).toBeNull();
    fireEvent.click(icon);
    expect(trash).toHaveBeenCalledOnce();
  });

  it("folds into More when the row is too narrow; a row runs its action and closes", () => {
    measuring(400, 100);
    const { list, end, next } = actions();
    renderUi(<OverflowActions label="More actions" actions={list} end={end} />);
    expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    const menu = screen.getByRole("dialog", { name: "More actions" });
    expect(menu).toBeVisible();
    // Each row says its name and its key.
    const row = screen.getByRole("button", { name: /^Next/u });
    expect(row).toHaveAttribute("aria-keyshortcuts");
    fireEvent.click(row);
    expect(next).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog", { name: "More actions" })).toBeNull();
  });

  it("moves between the menu's rows with the arrow keys", () => {
    measuring(400, 100);
    const { list, end } = actions();
    renderUi(<OverflowActions label="More actions" actions={list} end={end} />);
    fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    const first = screen.getByRole("button", { name: /^Delete/u });
    expect(first).toHaveFocus();
    fireEvent.keyDown(first, { key: "ArrowDown" });
    expect(screen.getByRole("button", { name: /^Pin/u })).toHaveFocus();
    fireEvent.keyDown(document.activeElement ?? first, { key: "ArrowUp" });
    fireEvent.keyDown(document.activeElement ?? first, { key: "ArrowUp" });
    expect(screen.getByRole("button", { name: /^Next/u })).toHaveFocus();
  });
});
