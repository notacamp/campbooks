import { fireEvent, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { useShortcut } from "~/lib/shortcuts";

import { renderUi } from "../../../../../test/render";

import { CommandMenu } from "./CommandMenu";

function Screen({
  onInbox,
  onAsk,
  onSuggestion,
}: {
  onInbox: () => void;
  onAsk?: (query: string) => void;
  onSuggestion?: () => void;
}) {
  const [open, setOpen] = useState(false);
  useShortcut("nav.inbox", onInbox);
  useShortcut("command.open", () => {
    setOpen(true);
  });
  return (
    <>
      <button type="button">before</button>
      <CommandMenu
        open={open}
        onClose={() => {
          setOpen(false);
        }}
        commands={[
          {
            id: "reply",
            label: "Réply to Sofia",
            group: "Suggested",
            shortcut: "item.reply",
            onSelect: onSuggestion ?? vi.fn(),
          },
        ]}
        onAsk={onAsk}
      />
    </>
  );
}

const open = () => {
  fireEvent.keyDown(document.body, { key: "k", ctrlKey: true });
};

describe("CommandMenu", () => {
  it("opens on Ctrl+K as a modal dialog focused on its input", () => {
    renderUi(<Screen onInbox={vi.fn()} />);
    open();

    expect(screen.getByRole("dialog", { name: "Command menu" })).toHaveAttribute(
      "aria-modal",
      "true",
    );
    expect(screen.getByRole("combobox")).toHaveFocus();
  });

  it("lists this screen's commands and every bound shortcut, grouped", () => {
    renderUi(<Screen onInbox={vi.fn()} />);
    open();

    expect(screen.getByRole("group", { name: "Suggested" })).toHaveTextContent("Réply to Sofia");
    expect(screen.getByRole("group", { name: "Go to" })).toHaveTextContent("InboxGthenI");
    expect(screen.queryByRole("option", { name: /Search or ask/ })).not.toBeInTheDocument();
  });

  it("filters ignoring accents and runs the active command with Enter", () => {
    const onInbox = vi.fn();
    renderUi(<Screen onInbox={onInbox} />);
    open();
    const input = screen.getByRole("combobox");

    fireEvent.change(input, { target: { value: "INB" } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onInbox).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("moves with the arrow keys, wrapping around", () => {
    renderUi(<Screen onInbox={vi.fn()} />);
    open();
    const input = screen.getByRole("combobox");
    const options = screen.getAllByRole("option");

    expect(options[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input).toHaveAttribute("aria-activedescendant", options[1]?.id);
    fireEvent.keyDown(input, { key: "ArrowUp" });
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(input).toHaveAttribute("aria-activedescendant", options.at(-1)?.id);
    fireEvent.keyDown(input, { key: "Tab" });
  });

  it("runs a command on click and hands a question to the assistant", () => {
    const onSuggestion = vi.fn();
    const onAsk = vi.fn();
    renderUi(<Screen onInbox={vi.fn()} onAsk={onAsk} onSuggestion={onSuggestion} />);
    open();
    fireEvent.mouseMove(screen.getByText("Réply to Sofia"));
    fireEvent.click(screen.getByText("Réply to Sofia"));
    expect(onSuggestion).toHaveBeenCalledOnce();

    open();
    fireEvent.change(screen.getByRole("combobox"), {
      target: { value: "What do I owe Brightloop?" },
    });
    fireEvent.click(
      screen.getByRole("option", { name: "Ask Campbooks: What do I owe Brightloop?" }),
    );
    expect(onAsk).toHaveBeenCalledWith("What do I owe Brightloop?");
  });

  it("says when nothing matches, and closes on Escape and on the scrim", () => {
    renderUi(<Screen onInbox={vi.fn()} />);
    open();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "zzz" } });
    expect(screen.getByText("Nothing matches “zzz”")).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });

    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    open();
    const scrim = screen.getByRole("dialog").parentElement;
    if (scrim !== null) {
      fireEvent.mouseDown(scrim);
    }
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
