import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { KICKOFF_NOTES } from "../../../../../test/demo/assistant";

import { PageMarkdown, parseBlocks } from "./markdown";

const labels = { done: "Done", open: "Not done" };

describe("PageMarkdown", () => {
  it("draws a section as the page would: heading, list, bold", () => {
    const { container } = render(<PageMarkdown markdown={KICKOFF_NOTES} todoLabels={labels} />);
    expect(
      screen.getByRole("heading", { level: 2, name: "Kickoff notes — Acme Studio" }),
    ).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")).toHaveLength(3);
    expect(container.querySelector("strong")?.textContent).toBe("Brand files");
  });

  it("keeps a link only to https, http or mailto, and drops the address of any other", () => {
    render(
      <PageMarkdown
        markdown="[site](https://example.com) [mail](mailto:ana@example.com) [bad](javascript:alert(1))"
        todoLabels={labels}
      />,
    );
    expect(screen.getByRole("link", { name: "site" })).toHaveAttribute(
      "href",
      "https://example.com",
    );
    expect(screen.getByRole("link", { name: "mail" })).toHaveAttribute(
      "href",
      "mailto:ana@example.com",
    );
    expect(screen.queryByRole("link", { name: "bad" })).not.toBeInTheDocument();
    expect(screen.getByText(/bad/u)).toBeInTheDocument();
  });

  it("treats raw HTML as text", () => {
    const { container } = render(
      <PageMarkdown
        markdown={"<img src=x onerror=alert(1)> <script>alert(1)</script>"}
        todoLabels={labels}
      />,
    );
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("script")).toBeNull();
    expect(container.textContent).toContain("<script>");
  });

  it("draws to-dos with a name for the box, numbered lists, quotes, code and rules", () => {
    const { container } = render(
      <PageMarkdown
        markdown={
          "- [x] Send the pack\n- [ ] Get the contract\n\n1. One\n2. Two\n\n> Keep it\n\n```\nlet a = 1;\n```\n\n---"
        }
        todoLabels={labels}
      />,
    );
    expect(screen.getByText("Done")).toBeInTheDocument();
    expect(screen.getByText("Not done")).toBeInTheDocument();
    expect(container.querySelector("ol")?.children).toHaveLength(2);
    expect(container.querySelector("blockquote")).not.toBeNull();
    expect(container.querySelector("pre code")?.textContent).toBe("let a = 1;");
    expect(container.querySelector("hr")).not.toBeNull();
  });
});

describe("parseBlocks", () => {
  it("never reads more headings than three levels", () => {
    expect(parseBlocks("##### Deep")).toEqual([{ kind: "heading", level: 3, text: "Deep" }]);
  });
});
