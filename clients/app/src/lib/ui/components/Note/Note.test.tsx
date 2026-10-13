import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Note } from "./Note";

describe("Note", () => {
  it("is a named region with linked records", () => {
    const onSelect = vi.fn();
    renderUi(
      <Note links={[{ icon: "check", title: "Review Q4 proposal", kind: "Task, Thu", onSelect }]}>
        Sofia is waiting on your review.
      </Note>,
    );

    fireEvent.click(screen.getByRole("button", { name: /Review Q4 proposal/ }));

    expect(screen.getByRole("region", { name: "Where things stand" })).toHaveTextContent(
      "Sofia is waiting",
    );
    expect(onSelect).toHaveBeenCalledOnce();
  });

  it("explains itself behind a Why? disclosure", () => {
    renderUi(
      <Note
        confidence="Sorted into Primary, 98% sure"
        why={["You have replied to Sofia 14 times this year."]}
      >
        Note text
      </Note>,
    );
    const why = screen.getByRole("button", { name: "Why?" });
    const reasons = screen.getByRole("list", { hidden: true });

    expect(why).toHaveAttribute("aria-expanded", "false");
    expect(reasons).not.toBeVisible();

    fireEvent.click(why);

    expect(screen.getByRole("button", { name: "Hide why" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(why).toHaveAttribute("aria-controls", reasons.id);
    expect(reasons).toBeVisible();
  });

  it("asks instead of acting when unsure", () => {
    const onPrimary = vi.fn();
    renderUi(
      <Note
        visor="unsure"
        compact
        label="Low confidence"
        choice={{
          prompt: "Where does this go?",
          options: [
            { label: "Primary", keys: "1", onSelect: onPrimary },
            { label: "Updates", keys: "2", onSelect: vi.fn() },
          ],
        }}
        footer={<span>Move</span>}
      >
        An invitation.
      </Note>,
    );

    fireEvent.click(screen.getByRole("button", { name: /Primary/ }));

    expect(onPrimary).toHaveBeenCalledOnce();
    expect(screen.getByRole("region", { name: "Low confidence" })).toHaveTextContent(
      "Where does this go?",
    );
    expect(screen.getByText("Move")).toBeInTheDocument();
  });
});
