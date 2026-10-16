import { act, fireEvent, screen } from "@testing-library/react";
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
    // The row stays calm: no bare key hints on it.
    expect(row).toHaveTextContent("Thu");
    expect(row.querySelector("kbd")).toBeNull();
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
        swipeActions={<SwipeAction tone="done" icon="check" label="Done" onClick={onDone} />}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Done" }));

    expect(screen.getByRole("link", { name: /Sofia Pessoa/ })).toHaveAttribute("href", "#c1");
    expect(onDone).toHaveBeenCalledOnce();
  });

  it("shows its actions under the row, outside the row's button", () => {
    renderUi(<Item {...base} selected actions={<button type="button">Reply</button>} />);
    const reply = screen.getByRole("button", { name: "Reply" });

    expect(screen.getByRole("button", { name: /Sofia Pessoa/ })).not.toContainElement(reply);
  });

  it("turns into a one-line confirmation that ends under a second", () => {
    vi.useFakeTimers();
    const onConfirmEnd = vi.fn();
    renderUi(
      <Item {...base} id="row-1" confirmation="Done · Sofia Pessoa" onConfirmEnd={onConfirmEnd} />,
    );

    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(screen.getByText("Done · Sofia Pessoa").parentElement).toHaveAttribute("id", "row-1");
    act(() => {
      vi.advanceTimersByTime(899);
    });
    expect(onConfirmEnd).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(onConfirmEnd).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });
});
