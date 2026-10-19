import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { FoldedMessage, MessageCard, MetaLine, MoreMessages, QuotedText } from "./Message";

describe("Message", () => {
  it("is an article with sender, time and attachment", () => {
    renderUi(
      <MessageCard
        from="Sofia Pessoa"
        time="Fri 17:42"
        dateTime="2026-10-09T17:42"
        attachment="scope.pdf"
      >
        Attached the revised scope.
      </MessageCard>,
    );
    const article = screen.getByRole("article");

    expect(article).toHaveTextContent("Sofia PessoaFri 17:42Attached the revised scope.");
    expect(screen.getByTitle("scope.pdf")).toBeInTheDocument();
    expect(article.querySelector("time")).toHaveAttribute("datetime", "2026-10-09T17:42");
  });

  it("sets yours apart and says who it went to", () => {
    renderUi(
      <MessageCard from="You" addressee="→ Marta Silva" side="yours" time="Thu 17:40">
        Revised quote attached.
      </MessageCard>,
    );
    const article = screen.getByRole("article");

    expect(article).toHaveAttribute("data-side", "yours");
    expect(article).toHaveTextContent("You → Marta SilvaThu 17:40Revised quote attached.");
  });

  it("is theirs by default, folded or open", () => {
    const onOpen = vi.fn();
    renderUi(
      <FoldedMessage
        from="Marta Silva"
        time="Tue"
        snippet="Could you send the quote?"
        onOpen={onOpen}
      />,
    );
    const folded = screen.getByRole("button", { name: /Marta Silva/u });

    expect(folded).toHaveAttribute("data-side", "theirs");
    expect(folded).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(folded);
    expect(onOpen).toHaveBeenCalledOnce();
  });

  it("folds the middle of a thread into one button", () => {
    const onOpen = vi.fn();
    renderUi(<MoreMessages label="4 more messages" onOpen={onOpen} />);

    fireEvent.click(screen.getByRole("button", { name: "4 more messages" }));
    expect(onOpen).toHaveBeenCalledOnce();
  });

  it("shows quoted text only when asked, and hides it again", () => {
    renderUi(
      <MessageCard
        from="Marta Silva"
        time="Fri 09:12"
        quoted={
          <QuotedText showLabel="Show quoted text" hideLabel="Hide quoted text">
            {"> Revised quote attached."}
          </QuotedText>
        }
      >
        Perfect, approved.
      </MessageCard>,
    );
    const quote = screen.getByText("> Revised quote attached.");
    expect(quote).not.toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Show quoted text" }));
    expect(quote).toBeVisible();
    const hide = screen.getByRole("button", { name: "Hide quoted text" });
    expect(hide).toHaveAttribute("aria-expanded", "true");

    fireEvent.click(hide);
    expect(quote).not.toBeVisible();
  });

  it("separates meta facts with decorative dots", () => {
    const { container } = renderUi(
      <MetaLine parts={["Sofia Pessoa", "Brightloop", "2 messages"]} />,
    );

    expect(container).toHaveTextContent("Sofia Pessoa·Brightloop·2 messages");
    expect(container.querySelectorAll("[aria-hidden='true']")).toHaveLength(2);
  });
});
