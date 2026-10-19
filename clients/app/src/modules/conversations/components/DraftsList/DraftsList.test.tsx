import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { NOW } from "../../../../../test/demo/conversations";
import { BLANK, DRAFT_LIST, REPLY_WRITTEN, TO_MARTA } from "../../../../../test/demo/drafts";
import { renderWithProviders } from "../../../../../test/render";
import { DRAFTS_MAX, type Draft } from "../../api/drafts";

import { DraftsList } from "./DraftsList";

function show(
  drafts: readonly Draft[] | undefined,
  handlers: { onOpen?: (draft: Draft) => void; onDelete?: (draft: Draft) => void } = {},
  failed = false,
) {
  return renderWithProviders(
    <DraftsList
      drafts={drafts}
      failed={failed}
      onRetry={vi.fn()}
      onOpen={handlers.onOpen ?? vi.fn()}
      onDelete={handlers.onDelete ?? vi.fn()}
      now={NOW}
    />,
  );
}

describe("the drafts list", () => {
  it("says who each draft is to, its subject and its words", () => {
    show(DRAFT_LIST);
    expect(screen.getByText("Draft · Maya Okafor")).toBeVisible();
    expect(screen.getByText("Draft · Marta Ferreira and 2 others")).toBeVisible();
    expect(screen.getByText("Draft · No recipients")).toBeVisible();
    expect(screen.getByText(/^Thanks, Maya\. I will go through/u)).toBeVisible();
    expect(screen.getByText("Figures for the board")).toBeVisible();
    expect(screen.getByText("(no subject)")).toBeVisible();
    expect(screen.queryByText(/newest drafts/u)).toBeNull();
  });

  it("opens the draft of the row pressed, and deletes the one beside it", () => {
    const onOpen = vi.fn();
    const onDelete = vi.fn();
    show(DRAFT_LIST, { onOpen, onDelete });
    fireEvent.click(screen.getByRole("button", { name: /Draft · Marta Ferreira/u }));
    expect(onOpen).toHaveBeenCalledWith(TO_MARTA);
    fireEvent.click(screen.getByRole("button", { name: "Delete draft: Figures for the board" }));
    expect(onDelete).toHaveBeenCalledWith(TO_MARTA);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it("leaves out a draft that is already on its way", () => {
    show([REPLY_WRITTEN, { ...TO_MARTA, state: "scheduled" }, BLANK]);
    expect(screen.queryByText("Figures for the board")).toBeNull();
    expect(screen.getByText("Draft · Maya Okafor")).toBeVisible();
  });

  it("says so when there are none", () => {
    show([]);
    expect(screen.getByRole("heading", { name: "No drafts." })).toBeVisible();
  });

  it("notes the cap only when it returns exactly that many", () => {
    const many = Array.from({ length: DRAFTS_MAX }, (_, index) => ({
      ...BLANK,
      id: `0192f1a0-0000-7000-8000-0000000002${String(index).padStart(2, "0")}`,
    }));
    const { unmount } = show(many);
    expect(screen.getByText("Your 50 newest drafts.")).toBeVisible();
    unmount();
    show(many.slice(1));
    expect(screen.queryByText("Your 50 newest drafts.")).toBeNull();
  });

  it("shows the loading and the error states", () => {
    const { unmount } = show(undefined, {}, false);
    expect(screen.queryByRole("heading")).toBeNull();
    unmount();
    show(undefined, {}, true);
    expect(screen.getByRole("heading", { name: "Could not load your drafts" })).toBeVisible();
  });
});
