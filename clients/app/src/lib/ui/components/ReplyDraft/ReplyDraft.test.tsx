import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { AskLine, ReplyDraft, ReplyQuote } from "./ReplyDraft";

describe("ReplyDraft", () => {
  it("is an editable text area that takes the focus, its text plain", () => {
    const onChange = vi.fn();
    renderUi(
      <ReplyDraft
        label="Your reply"
        value={"Hi Marta,\n\n<b>Yes</b>"}
        onChange={onChange}
        tools={<button type="button">Shorter</button>}
      />,
    );
    const text = screen.getByRole("textbox", { name: "Your reply" });
    expect(text).toHaveFocus();
    expect(text).toHaveValue("Hi Marta,\n\n<b>Yes</b>");
    expect(document.querySelector("b")).toBeNull();
    fireEvent.change(text, { target: { value: "Hi Marta, yes." } });
    expect(onChange).toHaveBeenCalledWith("Hi Marta, yes.");
    expect(screen.getByRole("button", { name: "Shorter" })).toBeInTheDocument();
  });

  it("keeps the text read-only and says what Scout is doing while busy", () => {
    renderUi(
      <ReplyDraft
        label="Your reply"
        value="Hi Marta"
        onChange={vi.fn()}
        busy
        busyLabel="Making it shorter…"
        tools={<button type="button">Shorter</button>}
      />,
    );
    const text = screen.getByRole("textbox", { name: "Your reply" });
    expect(text).toHaveAttribute("readonly");
    expect(text).toHaveAttribute("aria-busy", "true");
    expect(text).toHaveAccessibleDescription("Making it shorter…");
    expect(screen.queryByRole("button", { name: "Shorter" })).toBeNull();
  });
});

describe("AskLine", () => {
  it("sends the trimmed words and empties the field", () => {
    const onSubmit = vi.fn();
    renderUi(<AskLine label="Ask for a change" action="Change" onSubmit={onSubmit} />);
    const field = screen.getByRole("textbox", { name: "Ask for a change" });
    const button = screen.getByRole("button", { name: /Change/ });
    expect(button).toBeDisabled();
    fireEvent.change(field, { target: { value: "  warmer  " } });
    fireEvent.submit(field);
    expect(onSubmit).toHaveBeenCalledWith("warmer");
    expect(field).toHaveValue("");
  });

  it("asks nothing while disabled", () => {
    const onSubmit = vi.fn();
    renderUi(<AskLine label="Ask for a change" action="Change" disabled onSubmit={onSubmit} />);
    const field = screen.getByRole("textbox", { name: "Ask for a change" });
    expect(field).toBeDisabled();
    fireEvent.submit(field);
    expect(onSubmit).not.toHaveBeenCalled();
  });
});

describe("ReplyQuote", () => {
  it("quotes who said it and their words as text", () => {
    renderUi(<ReplyQuote from="Marta Ferreira, Mon 12 Oct" text="<script>x</script> Thursday?" />);
    expect(screen.getByText("Marta Ferreira, Mon 12 Oct")).toBeInTheDocument();
    expect(screen.getByText("<script>x</script> Thursday?")).toBeInTheDocument();
    expect(document.querySelector("script")).toBeNull();
  });
});
