import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { TaskRow } from "./TaskRow";

describe("TaskRow", () => {
  it("the check and the row are two buttons, each doing its own thing", () => {
    const onOpen = vi.fn();
    const onCheck = vi.fn();
    renderUi(
      <TaskRow
        title="Send the signed lease"
        kind="Do"
        meta="You wrote this"
        checkLabel="Mark done: Send the signed lease"
        onOpen={onOpen}
        onCheck={onCheck}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Mark done: Send the signed lease" }));
    expect(onCheck).toHaveBeenCalledTimes(1);
    expect(onOpen).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /^Do Send the signed lease/u }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it("marks the open task and shows its chips", () => {
    renderUi(
      <TaskRow
        title="EDP bill"
        kind="Pay"
        meta="From EDP · email"
        chips={[{ text: "Overdue · 2 days", tone: "danger" }]}
        checkLabel="Mark paid: EDP bill"
        current
        onOpen={vi.fn()}
        onCheck={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: /^Pay EDP bill/u })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(screen.getByText("Overdue · 2 days")).toBeInTheDocument();
  });

  it("puts one action at the end of the row, a button of its own", () => {
    const onOpen = vi.fn();
    const onAction = vi.fn();
    renderUi(
      <TaskRow
        title="Book the venue"
        kind="Do"
        meta="You wrote this"
        chips={[{ text: "With João" }]}
        action={
          <button type="button" onClick={onAction}>
            Take it back
          </button>
        }
        onOpen={onOpen}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Take it back" }));
    expect(onAction).toHaveBeenCalledTimes(1);
    expect(onOpen).not.toHaveBeenCalled();
  });
});
