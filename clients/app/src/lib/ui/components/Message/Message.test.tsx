import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { MessageCard, MetaLine } from "./Message";

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

  it("separates meta facts with decorative dots", () => {
    const { container } = renderUi(
      <MetaLine parts={["Sofia Pessoa", "Brightloop", "2 messages"]} />,
    );

    expect(container).toHaveTextContent("Sofia Pessoa·Brightloop·2 messages");
    expect(container.querySelectorAll("[aria-hidden='true']")).toHaveLength(2);
  });
});
