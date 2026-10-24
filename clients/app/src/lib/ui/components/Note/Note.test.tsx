import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";
import { HowDecided } from "../HowDecided/HowDecided";

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

  it("quotes the sender as plain text, never as markup", () => {
    renderUi(
      <Note
        quote={{ text: "<b>Click</b> https://example.com", attribution: "Marta, Mon 12 Oct" }}
        reasons={["The sender’s domain imitates yours."]}
      >
        Marta is waiting for your reply.
      </Note>,
    );
    const quote = screen.getByText("<b>Click</b> https://example.com");
    expect(quote.tagName).toBe("Q");
    expect(quote.querySelector("b")).toBeNull();
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.getByText("Marta, Mon 12 Oct")).toBeInTheDocument();
    expect(screen.getByText("The sender’s domain imitates yours.")).toBeInTheDocument();
  });

  it("keeps how it decided behind a disclosure", () => {
    renderUi(
      <Note
        how={<HowDecided rows={[{ question: "Needs a reply", answer: "Yes.", source: "0.90" }]} />}
      >
        Note text
      </Note>,
    );
    expect(screen.getByText("0.90")).not.toBeVisible();
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

  it("reads a titled note's sentence in the body font, with the title and its meta above", () => {
    renderUi(
      <Note title="Scout noticed" meta="1 of 5" after={<button type="button">See them</button>}>
        You got 34 emails from CI.
      </Note>,
    );
    const sentence = screen.getByText("You got 34 emails from CI.");
    expect(sentence).toHaveAttribute("data-body");
    expect(screen.getByText("Scout noticed")).toBeVisible();
    expect(screen.getByText("1 of 5")).toBeVisible();
    expect(screen.getByRole("button", { name: "See them" })).toBeVisible();
  });

  it("keeps the display line when there is no title", () => {
    renderUi(<Note meta="1 of 5">Sofia is waiting on your review.</Note>);
    expect(screen.getByText("Sofia is waiting on your review.")).not.toHaveAttribute("data-body");
    // Meta goes with a title only.
    expect(screen.queryByText("1 of 5")).toBeNull();
  });
});
