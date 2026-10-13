import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Composer } from "./Composer";

describe("Composer", () => {
  it("sends with Ctrl+Enter only while you are writing", () => {
    const onSend = vi.fn();
    const onDraft = vi.fn();
    const onChange = vi.fn();
    renderUi(
      <Composer
        label="Reply to Sofia"
        value=""
        onChange={onChange}
        onSend={onSend}
        onDraft={onDraft}
      />,
    );
    const box = screen.getByRole("textbox", { name: "Reply to Sofia" });

    fireEvent.keyDown(document.body, { key: "Enter", ctrlKey: true });
    expect(onSend).not.toHaveBeenCalled();

    fireEvent.focus(box);
    fireEvent.change(box, { target: { value: "Thursday works." } });
    fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    fireEvent.keyDown(box, { key: "j", ctrlKey: true });
    fireEvent.blur(box);

    expect(onChange).toHaveBeenCalledWith("Thursday works.");
    expect(onSend).toHaveBeenCalledOnce();
    expect(onDraft).toHaveBeenCalledOnce();
  });

  it("offers Draft and Send with their shortcuts", () => {
    renderUi(
      <Composer label="Reply" value="" onChange={vi.fn()} onSend={vi.fn()} onDraft={vi.fn()} />,
    );

    expect(screen.getByRole("button", { name: /Draft it for me/ })).toHaveAttribute(
      "aria-keyshortcuts",
      "Control+J",
    );
    expect(screen.getByRole("button", { name: /Send/ })).toHaveAttribute(
      "aria-keyshortcuts",
      "Control+Enter",
    );
  });

  it("hides Draft when there is no assistant to ask", () => {
    renderUi(<Composer label="Reply" value="" onChange={vi.fn()} onSend={vi.fn()} />);

    expect(screen.queryByRole("button", { name: /Draft/ })).not.toBeInTheDocument();
  });
});
