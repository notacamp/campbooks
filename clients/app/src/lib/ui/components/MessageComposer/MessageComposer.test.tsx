import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { editorOf, typeInto } from "../../../../../test/composer";
import { renderUi } from "../../../../../test/render";

import type { Recipient } from "../RecipientField/RecipientField";

import {
  MessageComposer,
  type MessageComposerProps,
  type RecipientSuggester,
} from "./MessageComposer";

function props(overrides: Partial<MessageComposerProps> = {}): MessageComposerProps {
  return {
    title: "Reply to Maya",
    from: "Sam Rivers <sam@example.com>",
    to: [{ name: "Maya", address: "maya@example.com" }],
    onToChange: vi.fn(),
    cc: [],
    onCcChange: vi.fn(),
    bcc: [],
    onBccChange: vi.fn(),
    body: "",
    onBodyChange: vi.fn(),
    onSend: vi.fn(),
    onClose: vi.fn(),
    ...overrides,
  };
}

describe("MessageComposer", () => {
  it("shows who it is from and to, and opens Cc and Bcc on demand", async () => {
    renderUi(<MessageComposer {...props()} />);
    expect(screen.getByRole("region", { name: "Reply to Maya" })).toBeInTheDocument();
    expect(screen.getByText("Sam Rivers <sam@example.com>")).toBeInTheDocument();
    expect(screen.getByText("Maya")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Cc" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cc Bcc" }));
    expect(screen.getByRole("textbox", { name: "Cc" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Bcc" })).toBeInTheDocument();
    // A reply keeps its subject: no field.
    expect(screen.queryByRole("textbox", { name: "Subject" })).not.toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Message" })).toHaveFocus();
    });
  });

  it("never hides copies, also ones that arrive after it opened", () => {
    function Later() {
      const [bcc, setBcc] = useState<MessageComposerProps["bcc"]>([]);
      return (
        <>
          <button
            type="button"
            onClick={() => {
              setBcc([{ name: null, address: "eve@example.com" }]);
            }}
          >
            Shown again
          </button>
          <MessageComposer {...props({ bcc })} />
        </>
      );
    }
    renderUi(<Later />);
    expect(screen.queryByRole("textbox", { name: "Bcc" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Shown again" }));
    expect(screen.getByRole("textbox", { name: "Bcc" })).toBeInTheDocument();
    expect(screen.getByText("eve@example.com")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cc Bcc" })).not.toBeInTheDocument();
  });

  it("sends with Ctrl+Enter, formats with Ctrl+B and closes with Escape, only while focused", () => {
    const onSend = vi.fn();
    const onClose = vi.fn();
    const onBodyChange = vi.fn();
    renderUi(<MessageComposer {...props({ onSend, onClose, onBodyChange, body: "see you" })} />);
    const box = screen.getByRole("textbox", { name: "Message" });
    fireEvent.blur(box);
    fireEvent.keyDown(document.body, { key: "Enter", ctrlKey: true });
    expect(onSend).not.toHaveBeenCalled();

    act(() => {
      box.focus();
      editorOf(box).commands.setTextSelection({ from: 5, to: 8 });
    });
    fireEvent.keyDown(box, { key: "b", ctrlKey: true });
    expect(onBodyChange).toHaveBeenLastCalledWith("see **you**");
    expect(box.querySelector("strong")).toHaveTextContent("you");
    fireEvent.keyDown(box, { key: "b", ctrlKey: true });
    expect(onBodyChange).toHaveBeenLastCalledWith("see you");
    fireEvent.keyDown(box, { key: "i", ctrlKey: true });
    expect(onBodyChange).toHaveBeenLastCalledWith("see *you*");
    fireEvent.keyDown(box, { key: "x", ctrlKey: true, shiftKey: true });
    expect(onBodyChange).toHaveBeenLastCalledWith("see ~~*you*~~");
    fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    expect(onSend).toHaveBeenCalledOnce();
    fireEvent.keyDown(box, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("suggests people in each field, and Esc closes the list before the composer", () => {
    const onClose = vi.fn();
    const onQueryChange = vi.fn();
    const suggest: RecipientSuggester = {
      suggestions: (field) =>
        field === "cc"
          ? [{ name: "Marta Ferreira", address: "marta@ferreira-studio.example" }]
          : [],
      loading: () => false,
      onQueryChange,
    };
    function Copying() {
      const [cc, setCc] = useState<Recipient[]>([]);
      return <MessageComposer {...props({ onClose, cc, onCcChange: setCc, suggest })} />;
    }
    renderUi(<Copying />);
    fireEvent.click(screen.getByRole("button", { name: "Cc Bcc" }));
    const cc = screen.getByRole("combobox", { name: "Cc" });
    act(() => {
      cc.focus();
    });
    fireEvent.change(cc, { target: { value: "mar" } });
    expect(onQueryChange).toHaveBeenLastCalledWith("cc", "mar");
    expect(screen.getByRole("listbox", { name: "Suggestions for Cc" })).toBeVisible();
    expect(screen.getByRole("combobox", { name: "To" })).toHaveAttribute("aria-expanded", "false");

    fireEvent.keyDown(cc, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(cc, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();

    fireEvent.keyDown(cc, { key: "ArrowDown" });
    fireEvent.keyDown(cc, { key: "Enter" });
    expect(screen.getByText("Marta Ferreira")).toBeVisible();
    expect(onQueryChange).toHaveBeenLastCalledWith("cc", "");
  });

  it("formats from the toolbar, shows it as formatting, and offers its shortcuts", () => {
    const onBodyChange = vi.fn();
    renderUi(<MessageComposer {...props({ onBodyChange, body: "budget" })} />);
    const toolbar = screen.getByRole("toolbar", { name: "Formatting" });
    expect(within(toolbar).getByRole("button", { name: "Bold" })).toHaveAttribute(
      "aria-keyshortcuts",
      "Control+B",
    );
    for (const [name, keys] of [
      ["Strikethrough", "Control+Shift+X"],
      ["Link", "Control+Shift+K"],
      ["Bulleted list", "Control+Shift+8"],
      ["Numbered list", "Control+Shift+7"],
      ["Quote", "Control+Shift+9"],
    ] as const) {
      expect(within(toolbar).getByRole("button", { name })).toHaveAttribute(
        "aria-keyshortcuts",
        keys,
      );
    }
    fireEvent.click(within(toolbar).getByRole("button", { name: "Bulleted list" }));
    expect(onBodyChange).toHaveBeenLastCalledWith("- budget");
    expect(
      screen.getByRole("textbox", { name: "Message" }).querySelector("ul li"),
    ).toHaveTextContent("budget");
    expect(within(toolbar).getByRole("button", { name: "Bulleted list" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(within(toolbar).getByRole("button", { name: "Numbered list" }));
    expect(onBodyChange).toHaveBeenLastCalledWith("1. budget");
    expect(screen.getByRole("button", { name: /Send/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "Control+Enter",
    );
  });

  it("quotes a paragraph from the toolbar", () => {
    const onBodyChange = vi.fn();
    renderUi(<MessageComposer {...props({ onBodyChange, body: "budget" })} />);
    fireEvent.click(screen.getByRole("button", { name: "Quote" }));
    expect(onBodyChange).toHaveBeenLastCalledWith("> budget");
  });

  it("opens a draft's marks as formatting, and types a literal asterisk as one", () => {
    const onBodyChange = vi.fn();
    renderUi(
      <MessageComposer
        {...props({
          onBodyChange,
          body: "- parking P2-14\n- rent on the ~~5th~~ 1st",
        })}
      />,
    );
    const box = screen.getByRole("textbox", { name: "Message" });
    expect(box.querySelectorAll("ul > li")).toHaveLength(2);
    expect(box.querySelector("s")).toHaveTextContent("5th");
    act(() => {
      editorOf(box).commands.setContent("<p></p>", { emitUpdate: false });
    });
    typeInto(box, "2 * 3 = 6");
    expect(onBodyChange).toHaveBeenLastCalledWith(String.raw`2 \* 3 = 6`);
  });

  it("keeps only what the schema holds when content is pasted", () => {
    const onBodyChange = vi.fn();
    renderUi(<MessageComposer {...props({ onBodyChange })} />);
    const box = screen.getByRole("textbox", { name: "Message" });
    act(() => {
      editorOf(box).commands.insertContent(
        '<h2>Heading</h2><img src="https://example.com/a.png"><p>A <b>bold</b> sentence</p>',
      );
    });
    expect(box.querySelector("h2, img")).toBeNull();
    expect(onBodyChange).toHaveBeenLastCalledWith("Heading\n\nA **bold** sentence");
  });

  it("links words to an address, and refuses any that is not web or mail", () => {
    const onBodyChange = vi.fn();
    renderUi(<MessageComposer {...props({ onBodyChange, body: "See the lease." })} />);
    const box = screen.getByRole("textbox", { name: "Message" });
    act(() => {
      box.focus();
      editorOf(box).commands.setTextSelection({ from: 5, to: 14 });
    });
    fireEvent.keyDown(box, { key: "k", ctrlKey: true, shiftKey: true });
    const address = screen.getByRole("textbox", { name: "Address" });
    expect(screen.getByRole("textbox", { name: "Words" })).toHaveValue("the lease");
    fireEvent.change(address, { target: { value: "javascript:alert(1)" } });
    fireEvent.click(screen.getByRole("button", { name: "Add link" }));
    expect(screen.getByText("Only web and email addresses can be linked.")).toBeVisible();
    expect(onBodyChange).not.toHaveBeenCalled();
    fireEvent.change(address, { target: { value: "https://example.com/4b" } });
    fireEvent.click(screen.getByRole("button", { name: "Add link" }));
    expect(onBodyChange).toHaveBeenLastCalledWith("See [the lease](https://example.com/4b).");
    expect(box.querySelector("a")).toHaveAttribute("href", "https://example.com/4b");
    expect(screen.queryByRole("textbox", { name: "Address" })).not.toBeInTheDocument();
  });

  it("closes the link form with Escape before it closes the composer", () => {
    const onClose = vi.fn();
    renderUi(<MessageComposer {...props({ onClose })} />);
    const box = screen.getByRole("textbox", { name: "Message" });
    act(() => {
      box.focus();
    });
    fireEvent.keyDown(box, { key: "k", ctrlKey: true, shiftKey: true });
    const words = screen.getByRole("textbox", { name: "Words" });
    act(() => {
      words.focus();
    });
    fireEvent.keyDown(words, { key: "Escape" });
    expect(screen.queryByRole("textbox", { name: "Words" })).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("takes the server's echo only when it differs, so a save never moves the cursor", () => {
    function Echo({ body }: { body: string }) {
      return <MessageComposer {...props({ body })} />;
    }
    const { rerender } = renderUi(<Echo body="Thursday works." />);
    const box = screen.getByRole("textbox", { name: "Message" });
    act(() => {
      editorOf(box).commands.setTextSelection(4);
    });
    // The echo of what is on screen changes nothing.
    rerender(<Echo body="Thursday works." />);
    expect(editorOf(box).state.selection.from).toBe(4);
    // A different body (the draft shown again as the server holds it) does.
    rerender(<Echo body="Friday instead." />);
    expect(screen.getByRole("textbox", { name: "Message" })).toHaveTextContent("Friday instead.");
  });

  it("expands into a sheet and back without losing the cursor or the undo history", () => {
    renderUi(<MessageComposer {...props({ body: "Thursday works." })} />);
    const box = screen.getByRole("textbox", { name: "Message" });
    act(() => {
      box.focus();
      editorOf(box).commands.setTextSelection(9);
    });
    typeInto(box, "!");
    const frame = box.closest("dialog");
    expect(frame).not.toHaveAttribute("data-expanded");
    fireEvent.keyDown(box, { key: "e", ctrlKey: true, shiftKey: true });
    expect(frame).toHaveAttribute("data-expanded");
    expect(screen.getByRole("button", { name: "Shrink" })).toBeVisible();
    // The same element, text and cursor.
    expect(screen.getByRole("textbox", { name: "Message" })).toBe(box);
    expect(editorOf(box).state.selection.from).toBe(10);
    typeInto(box, "?");
    // Esc shrinks it (it does not close the composer).
    fireEvent.keyDown(box, { key: "Escape" });
    expect(frame).not.toHaveAttribute("data-expanded");
    expect(box).toHaveTextContent("Thursday!? works.");
    expect(editorOf(box).can().undo()).toBe(true);
    act(() => {
      editorOf(box).commands.undo();
    });
    expect(box).not.toHaveTextContent("?");
  });

  it("hides Expand on a phone", () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: false,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    renderUi(<MessageComposer {...props()} />);
    expect(screen.queryByRole("button", { name: "Expand" })).not.toBeInTheDocument();
    vi.unstubAllGlobals();
  });

  it("edits the subject, the folded quote and the attachments of a forward", () => {
    const onSubjectChange = vi.fn();
    const onQuotedChange = vi.fn();
    const onRemoveAttachment = vi.fn();
    const onDiscard = vi.fn();
    renderUi(
      <MessageComposer
        {...props({
          title: "Forward",
          to: [],
          focus: "to",
          subject: "Fwd: Plan",
          onSubjectChange,
          quoted: "---------- Forwarded message ----------",
          onQuotedChange,
          attachments: [{ id: "a1", filename: "plan.pdf", size: 2048 }],
          onRemoveAttachment,
          onDiscard,
          status: "Saved",
          error: "Gmail refused it.",
        })}
      />,
    );
    expect(screen.getByRole("textbox", { name: "To" })).toHaveFocus();
    fireEvent.change(screen.getByRole("textbox", { name: "Subject" }), {
      target: { value: "Fwd: Plan v2" },
    });
    expect(onSubjectChange).toHaveBeenCalledWith("Fwd: Plan v2");
    fireEvent.click(screen.getByRole("button", { name: "Show quoted text" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Quoted text" }), {
      target: { value: "" },
    });
    expect(onQuotedChange).toHaveBeenCalledWith("");
    expect(screen.getByText("2 kB")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Remove plan.pdf" }));
    expect(onRemoveAttachment).toHaveBeenCalledWith("a1");
    expect(screen.getByRole("alert")).toHaveTextContent("Gmail refused it.");
    expect(screen.getByText("Saved")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(onDiscard).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Hide quoted text" }));
    expect(screen.queryByRole("textbox", { name: "Quoted text" })).not.toBeInTheDocument();
  });

  it("does not send twice while sending", () => {
    const onSend = vi.fn();
    renderUi(<MessageComposer {...props({ onSend, sending: true })} />);
    expect(screen.getByRole("button", { name: /Send/u })).toBeDisabled();
    act(() => {
      fireEvent.keyDown(screen.getByRole("textbox", { name: "Message" }), {
        key: "Enter",
        ctrlKey: true,
      });
    });
    expect(onSend).not.toHaveBeenCalled();
  });
});
