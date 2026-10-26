import { act, fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ShortcutRegistry } from "../../shortcuts";

import { renderUi } from "../../../test/render";
import { TEST_SHORTCUTS } from "../../../test/shortcuts";

import { ShortcutSheet } from "./ShortcutSheet";

describe("ShortcutSheet", () => {
  it("lists every registered shortcut by group with its announced keys", () => {
    renderUi(<ShortcutSheet open onClose={vi.fn()} />);
    const dialog = screen.getByRole("dialog", { name: "Keyboard shortcuts" });

    expect(within(dialog).getByRole("heading", { name: "Go to" })).toBeInTheDocument();
    expect(within(dialog).getByText("Inbox").closest("div")).toHaveTextContent(
      "Shortcut: G then I",
    );
    expect(within(dialog).getByText("Search or ask").closest("div")).toHaveTextContent(
      "Shortcut: Ctrl K",
    );
  });

  it("lists a focus page's shortcut only while that page binds it", () => {
    const registry = new ShortcutRegistry([
      ...TEST_SHORTCUTS,
      { id: "page.go", keys: "3", scope: "focus", group: "general", label: "shortcuts:undo" },
    ]);
    renderUi(<ShortcutSheet open onClose={vi.fn()} />, { registry });
    expect(screen.getAllByText("Undo")).toHaveLength(1);

    act(() => {
      registry.bind("page.go", vi.fn());
    });
    expect(screen.getAllByText("Undo")).toHaveLength(2);
  });

  it("is translated", () => {
    renderUi(<ShortcutSheet open onClose={vi.fn()} />, { locale: "es-ES" });

    expect(screen.getByRole("dialog", { name: "Atajos de teclado" })).toHaveTextContent(
      "Bandeja de entrada",
    );
  });

  it("closes on Escape and traps Tab inside", () => {
    const onClose = vi.fn();
    renderUi(
      <>
        <button type="button">outside</button>
        <ShortcutSheet open onClose={onClose} />
      </>,
    );
    const dialog = screen.getByRole("dialog");

    fireEvent.keyDown(dialog, { key: "Tab" });
    fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true });
    fireEvent.keyDown(dialog, { key: "Escape" });

    expect(onClose).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "outside" })).not.toHaveFocus();
  });

  it("closes from its close button", () => {
    const onClose = vi.fn();
    renderUi(<ShortcutSheet open onClose={onClose} />);

    fireEvent.click(screen.getByRole("button", { name: "Close" }));

    expect(onClose).toHaveBeenCalledOnce();
  });
});
