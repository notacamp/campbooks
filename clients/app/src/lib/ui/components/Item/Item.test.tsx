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

  it("reads its eyebrow first: the kind, then why it is here", () => {
    renderUi(
      <Item
        {...base}
        eyebrow={{ signal: "pay", kind: "Pay task", reason: "Overdue · 2 days", danger: true }}
      />,
    );
    const row = screen.getByRole("button", { name: /Sofia Pessoa/u });

    expect(row).toHaveAttribute("data-eyebrow");
    expect(row.textContent).toMatch(/Pay task · Overdue · 2 days Sofia Pessoa/u);
    expect(screen.getByText("Overdue · 2 days")).toHaveAttribute("data-danger", "true");
  });

  it("says why it is here under the visor, with its notes", () => {
    renderUi(
      <ItemDetail
        why={{
          label: "Why it’s here",
          text: "In Now because overdue · 2 days.",
          notes: ["Scam warnings and anything overdue."],
        }}
        actions={<button type="button">Open</button>}
      />,
    );
    const why = screen.getByRole("region", { name: "Why it’s here" });

    expect(why).toHaveTextContent("In Now because overdue · 2 days.");
    expect(why).toHaveTextContent("Scam warnings and anything overdue.");
  });

  it("draws an event's kind in place of an avatar, with no dot in its eyebrow", () => {
    const { container } = renderUi(
      <Item
        kind="meeting"
        eyebrow={{ kind: "Meeting", reason: "In 20 min" }}
        who="Call with Marta Silva"
        what="meet.example.com"
        due="11:20 – 11:50"
      />,
    );
    const row = screen.getByRole("button", { name: /Call with Marta Silva/u });

    expect(row.querySelector("[data-kind='meeting'][data-size='md']")).not.toBeNull();
    expect(row.querySelector("[data-avatar-hue]")).toBeNull();
    expect(container.querySelector("[data-signal]")).toBeNull();
    expect(row.textContent).toMatch(/Meeting · In 20 min Call with Marta Silva 11:20 – 11:50/u);
  });

  it("draws no eyebrow without one", () => {
    renderUi(<Item {...base} />);

    expect(screen.getByRole("button", { name: /Sofia Pessoa/u })).not.toHaveAttribute(
      "data-eyebrow",
    );
  });

  it("shows Scout's summary under its heading, before the email, as text only", () => {
    renderUi(
      <ItemDetail
        summaryLabel="Scout’s summary"
        summary="Marta asks you to confirm Thursday. <b>Not bold</b>"
        mail={<p>The email</p>}
        actions={<button type="button">Open</button>}
      />,
    );
    const summary = screen.getByRole("region", { name: "Scout’s summary" });

    expect(summary).toHaveTextContent("Marta asks you to confirm Thursday. <b>Not bold</b>");
    expect(summary.querySelector("b")).toBeNull();
    expect(summary.querySelector("[data-size]")).not.toBeNull();
    const text = document.body.textContent;
    expect(text.indexOf("Marta asks")).toBeLessThan(text.indexOf("The email"));
  });

  it("draws no summary without one, and the email still shows", () => {
    renderUi(
      <ItemDetail
        summaryLabel="Scout’s summary"
        mail={<p>The email</p>}
        actions={<button type="button">Open</button>}
      />,
    );

    expect(screen.queryByRole("region")).not.toBeInTheDocument();
    expect(screen.getByText("The email")).toBeInTheDocument();
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
