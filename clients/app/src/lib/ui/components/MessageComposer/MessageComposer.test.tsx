import { act, fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { MessageComposer, type MessageComposerProps } from "./MessageComposer";

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
  it("shows who it is from and to, and opens Cc and Bcc on demand", () => {
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
    expect(screen.getByRole("textbox", { name: "Message" })).toHaveFocus();
  });

  it("sends with Ctrl+Enter, formats with Ctrl+B and closes with Escape, only while focused", () => {
    const onSend = vi.fn();
    const onClose = vi.fn();
    const onBodyChange = vi.fn();
    renderUi(<MessageComposer {...props({ onSend, onClose, onBodyChange, body: "see you" })} />);
    const box = screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Message" });
    fireEvent.blur(box);
    fireEvent.keyDown(document.body, { key: "Enter", ctrlKey: true });
    expect(onSend).not.toHaveBeenCalled();

    fireEvent.focus(box);
    box.setSelectionRange(4, 7);
    fireEvent.keyDown(box, { key: "b", ctrlKey: true });
    expect(onBodyChange).toHaveBeenCalledWith("see **you**");
    fireEvent.keyDown(box, { key: "i", ctrlKey: true });
    expect(onBodyChange).toHaveBeenCalledWith("see *you*");
    fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    expect(onSend).toHaveBeenCalledOnce();
    fireEvent.keyDown(box, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("formats from the toolbar, and offers its shortcuts", () => {
    const onBodyChange = vi.fn();
    renderUi(<MessageComposer {...props({ onBodyChange, body: "budget" })} />);
    vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation((step) => {
      step(0);
      return 0;
    });
    expect(screen.getByRole("button", { name: "Bold" })).toHaveAttribute(
      "aria-keyshortcuts",
      "Control+B",
    );
    fireEvent.mouseDown(screen.getByRole("button", { name: "Bulleted list" }));
    fireEvent.click(screen.getByRole("button", { name: "Bulleted list" }));
    expect(onBodyChange).toHaveBeenCalledWith("- budget");
    fireEvent.click(screen.getByRole("button", { name: "Numbered list" }));
    expect(onBodyChange).toHaveBeenCalledWith("1. budget");
    expect(screen.getByRole("button", { name: /Send/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "Control+Enter",
    );
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
