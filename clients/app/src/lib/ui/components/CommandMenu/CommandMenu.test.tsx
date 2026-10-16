import { fireEvent, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { useShortcut } from "~/lib/shortcuts";

import { renderUi } from "../../../../../test/render";

import { CommandMenu, type MenuQuery, type MenuSection } from "./CommandMenu";

function Screen({
  onInbox,
  onAsk,
  useSection,
  onSuggestion,
}: {
  onInbox: () => void;
  onAsk?: (query: string) => void;
  useSection?: (menu: MenuQuery) => MenuSection | undefined;
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
        useSection={useSection}
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

  describe("with a section above the commands", () => {
    const hits = ["Lease renewal", "Lease inspection"];
    const onHit = vi.fn();
    const onAll = vi.fn();
    const onChip = vi.fn();
    const useHits = ({ query }: MenuQuery): MenuSection | undefined =>
      query.trim() === ""
        ? undefined
        : {
            group: "Conversations",
            chips: [{ id: "from:ana", label: "From ana", onRemove: onChip }],
            onSubmitAll: onAll,
            options: [
              ...hits.map((hit) => ({
                id: hit,
                label: hit,
                onSelect: () => {
                  onHit(hit);
                },
              })),
              { id: "all", label: `All results for “${query.trim()}”`, onSelect: onAll },
            ],
          };

    it("lists it first, then the commands, then Ask, with one cursor across them", () => {
      renderUi(<Screen onInbox={vi.fn()} onAsk={vi.fn()} useSection={useHits} />);
      open();
      const input = screen.getByRole("combobox");

      fireEvent.change(input, { target: { value: "lease" } });
      const options = screen.getAllByRole("option");

      expect(options.map((option) => option.textContent)).toEqual([
        "Lease renewal",
        "Lease inspection",
        "All results for “lease”",
        "Ask Campbooks: lease",
      ]);
      expect(options[0]).toHaveAttribute("aria-selected", "true");
      fireEvent.keyDown(input, { key: "ArrowDown" });
      fireEvent.keyDown(input, { key: "j", ctrlKey: true });
      fireEvent.keyDown(input, { key: "ArrowDown" });
      expect(input).toHaveAttribute("aria-activedescendant", options[3]?.id);
      fireEvent.keyDown(input, { key: "ArrowDown" });
      expect(options[0]).toHaveAttribute("aria-selected", "true");
      expect(options.every((option) => option.getAttribute("tabindex") === "-1")).toBe(true);
    });

    it("runs a row with Enter or a click, and ⌘↵ shows everything", () => {
      renderUi(<Screen onInbox={vi.fn()} useSection={useHits} />);
      open();
      const input = screen.getByRole("combobox");
      fireEvent.change(input, { target: { value: "lease" } });

      fireEvent.keyDown(input, { key: "ArrowDown" });
      fireEvent.keyDown(input, { key: "Enter" });
      expect(onHit).toHaveBeenLastCalledWith("Lease inspection");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

      open();
      fireEvent.change(screen.getByRole("combobox"), { target: { value: "lease" } });
      fireEvent.click(screen.getByRole("option", { name: "Lease renewal" }));
      expect(onHit).toHaveBeenLastCalledWith("Lease renewal");

      open();
      fireEvent.change(screen.getByRole("combobox"), { target: { value: "lease" } });
      fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter", ctrlKey: true });
      expect(onAll).toHaveBeenCalledOnce();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("shows its chips under the input, and removing one keeps the focus in the field", () => {
      renderUi(<Screen onInbox={vi.fn()} useSection={useHits} />);
      open();
      const input = screen.getByRole("combobox");
      fireEvent.change(input, { target: { value: "lease" } });

      fireEvent.click(screen.getByRole("button", { name: "Remove filter: From ana" }));

      expect(onChip).toHaveBeenCalledOnce();
      expect(input).toHaveFocus();
    });

    it("hints ⌘↵ only when the section can show everything", () => {
      const { unmount } = renderUi(<Screen onInbox={vi.fn()} useSection={useHits} />);
      open();
      fireEvent.change(screen.getByRole("combobox"), { target: { value: "lease" } });
      expect(screen.getByText("All results")).toBeInTheDocument();
      unmount();

      renderUi(<Screen onInbox={vi.fn()} />);
      open();
      expect(screen.queryByText("All results")).not.toBeInTheDocument();
      expect(screen.getByText("Move")).toBeInTheDocument();
      expect(screen.getByText("Close")).toBeInTheDocument();
    });
  });

  it("keeps a section's rows open after they run when they say so", () => {
    const fill = vi.fn();
    const useRecent = (): MenuSection => ({
      group: "Recent searches",
      options: [{ id: "lease", label: "lease", keepOpen: true, onSelect: fill }],
    });
    renderUi(<Screen onInbox={vi.fn()} useSection={useRecent} />);
    open();

    fireEvent.click(screen.getByRole("option", { name: "lease" }));

    expect(fill).toHaveBeenCalledOnce();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
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
