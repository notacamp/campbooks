import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { ExplanationCard, ExplanationLine, type ExplanationGroup } from "./Explanation";

const onChip = vi.fn();
const groups: ExplanationGroup[] = [
  {
    id: "wants",
    label: "Who wants what",
    folded: true,
    items: [
      {
        id: "w1",
        lead: "Jonas Weber",
        text: "a revised quote",
        chips: [{ id: "c1", label: "Jonas · 29 Oct", onSelect: onChip }],
      },
    ],
  },
  {
    id: "on-you",
    label: "On you",
    emphasis: true,
    items: [
      {
        id: "y1",
        text: "Send Jonas the revised quote.",
        due: "by Thu 6 Nov",
        chips: [{ id: "c2", label: "Jonas · 29 Oct", onSelect: onChip }],
      },
    ],
  },
];

const base = { title: "Scout's explanation", hideLabel: "Hide the explanation", onHide: vi.fn() };

describe("ExplanationCard", () => {
  it("shows the summary, the groups and the messages each point comes from", () => {
    renderUi(
      <ExplanationCard
        {...base}
        meta="From 9 messages · written today 10:42"
        inShort={{ label: "In short", text: "Acme wants the kickoff moved." }}
        groups={groups}
        actions={<button type="button">Ask a follow-up</button>}
        privacy="Only you see this."
      />,
    );
    const card = screen.getByRole("region", { name: "Scout's explanation" });
    expect(card).not.toHaveAttribute("aria-busy");
    expect(within(card).getByText("Acme wants the kickoff moved.")).toBeInTheDocument();
    expect(within(card).getByText("From 9 messages · written today 10:42")).toBeInTheDocument();
    expect(
      within(card).getByText(/Send Jonas the revised quote\. by Thu 6 Nov/u),
    ).toBeInTheDocument();
    expect(within(card).getByText("Only you see this.")).toBeInTheDocument();
    fireEvent.click(within(card).getAllByRole("button", { name: "Jonas · 29 Oct" }).at(-1) ?? card);
    expect(onChip).toHaveBeenCalledTimes(1);
    fireEvent.click(within(card).getByRole("button", { name: "Hide the explanation" }));
    expect(base.onHide).toHaveBeenCalledTimes(1);
  });

  it("draws everything as plain text, never as markup or a link", () => {
    renderUi(
      <ExplanationCard
        {...base}
        inShort={{
          label: "In short",
          text: "<b>pay now</b> [click](https://evil.example) https://evil.example",
        }}
        groups={[
          {
            id: "open",
            label: "Still open",
            items: [{ id: "o1", text: "<img src=x onerror=alert(1)>", chips: [] }],
          },
        ]}
      />,
    );
    const card = screen.getByRole("region", { name: "Scout's explanation" });
    expect(card.querySelector("b, img, a")).toBeNull();
    expect(within(card).getByText(/<b>pay now<\/b>/u)).toBeInTheDocument();
    expect(within(card).getByText("<img src=x onerror=alert(1)>")).toBeInTheDocument();
  });

  it("is a skeleton with Stop while Scout reads", () => {
    const onStop = vi.fn();
    renderUi(
      <ExplanationCard
        {...base}
        writing={{
          label: "Scout is reading the 9 messages…",
          stopLabel: "Stop",
          onStop,
          inShortLabel: "In short",
        }}
      />,
    );
    const card = screen.getByRole("region", { name: "Scout's explanation" });
    expect(card).toHaveAttribute("aria-busy", "true");
    expect(within(card).getByText("Scout is reading the 9 messages…")).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: "Hide the explanation" })).toBeNull();
    fireEvent.click(within(card).getByRole("button", { name: "Stop" }));
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("says why in a notice, above the content", () => {
    renderUi(
      <ExplanationCard
        {...base}
        notice="Rui Matos wrote since, at 11:05."
        inShort={{ label: "In short", text: "Kept." }}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("Rui Matos wrote since, at 11:05.");
  });

  it("folds the other groups behind a button for a phone", () => {
    renderUi(
      <ExplanationCard
        {...base}
        inShort={{ label: "In short", text: "Kept." }}
        groups={groups}
        more={{ label: "Who wants what, agreed, open", lessLabel: "Show less" }}
      />,
    );
    const more = screen.getByText("Who wants what, agreed, open");
    expect(more).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(more);
    expect(screen.getByText("Show less")).toHaveAttribute("aria-expanded", "true");
  });
});

describe("ExplanationLine", () => {
  it("is one line with its buttons", () => {
    renderUi(
      <ExplanationLine actions={<button type="button">Show</button>}>
        Your explanation from 10:42 · from 9 messages
      </ExplanationLine>,
    );
    expect(screen.getByText("Your explanation from 10:42 · from 9 messages")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show" })).toBeInTheDocument();
  });
});
