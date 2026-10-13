import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Item, ItemDetail, SwipeAction } from "./Item";

const base = {
  who: "Sofia Pessoa",
  what: "Waiting on your review",
  avatar: { name: "Sofia Pessoa" },
};

describe("Item", () => {
  it("expands its detail and says so", () => {
    const onToggle = vi.fn();
    renderUi(
      <Item
        {...base}
        due="Thu"
        tags={["2 messages"]}
        selected
        expanded
        onToggle={onToggle}
        detail={
          <ItemDetail
            quoteFrom="Sofia, Friday"
            quote="Attached the scope."
            context="You held Thursday."
            actions={<button type="button">Reply</button>}
          />
        }
      />,
    );
    const row = screen.getByRole("button", { name: /Sofia Pessoa/ });

    fireEvent.click(row);

    expect(onToggle).toHaveBeenCalledOnce();
    expect(row).toHaveAttribute("aria-expanded", "true");
    expect(row).toHaveAttribute("data-selected", "true");
    expect(row).toHaveTextContent(/Thu ?EL↵/); // due, then the done, later and open hints
    expect(screen.getByText("Attached the scope.")).toBeInTheDocument();
    expect(screen.getByText("You held Thursday.")).toBeInTheDocument();
  });

  it("keeps the detail out of the page while collapsed", () => {
    renderUi(<Item {...base} detail={<p>detail</p>} />);

    expect(screen.getByRole("button")).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("detail")).not.toBeInTheDocument();
  });

  it("links to a page on phones, with swipe actions", () => {
    const onDone = vi.fn();
    renderUi(
      <Item
        {...base}
        href="#c1"
        soon
        hints={[]}
        swipeActions={<SwipeAction tone="done" icon="check" label="Done" onClick={onDone} />}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Done" }));

    expect(screen.getByRole("link", { name: /Sofia Pessoa/ })).toHaveAttribute("href", "#c1");
    expect(onDone).toHaveBeenCalledOnce();
  });
});
