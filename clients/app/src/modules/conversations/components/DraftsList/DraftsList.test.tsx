import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { NOW } from "../../../../../test/demo/conversations";
import { BLANK, DRAFT_LIST, REPLY_WRITTEN, TO_MARTA } from "../../../../../test/demo/drafts";
import { renderWithProviders } from "../../../../../test/render";
import { DRAFTS_MAX, type Draft } from "../../api/drafts";

import { DraftsList } from "./DraftsList";

function show(
  drafts: readonly Draft[] | undefined,
  handlers: {
    onOpen?: (draft: Draft) => void;
    onDelete?: (draft: Draft) => void;
    onCancelSend?: (draft: Draft) => void;
    onCancelAndEdit?: (draft: Draft) => void;
  } = {},
  failed = false,
) {
  return renderWithProviders(
    <DraftsList
      drafts={drafts}
      failed={failed}
      onRetry={vi.fn()}
      onOpen={handlers.onOpen ?? vi.fn()}
      onDelete={handlers.onDelete ?? vi.fn()}
      onCancelSend={handlers.onCancelSend ?? vi.fn()}
      onCancelAndEdit={handlers.onCancelAndEdit ?? vi.fn()}
      now={NOW}
    />,
  );
}

describe("the drafts list", () => {
  it("says who each draft is to, its subject and its words", () => {
    show(DRAFT_LIST);
    expect(screen.getByText("Maya Okafor")).toBeVisible();
    expect(screen.getByText("Marta Ferreira +2")).toBeVisible();
    expect(screen.getByText("No recipients")).toBeVisible();
    expect(screen.getByText(/^Thanks, Maya\. I will go through/u)).toBeVisible();
    expect(screen.getByText("Figures for the board")).toBeVisible();
    expect(screen.getByText("(no subject)")).toBeVisible();
    expect(screen.getAllByText("Draft")).toHaveLength(3);
    expect(screen.queryByText(/newest drafts/u)).toBeNull();
  });

  it("opens the draft of the row pressed, and deletes the one beside it", () => {
    const onOpen = vi.fn();
    const onDelete = vi.fn();
    show(DRAFT_LIST, { onOpen, onDelete });
    fireEvent.click(screen.getByRole("button", { name: /Marta Ferreira/u }));
    expect(onOpen).toHaveBeenCalledWith(TO_MARTA);
    fireEvent.click(screen.getByRole("button", { name: "Delete draft: Figures for the board" }));
    expect(onDelete).toHaveBeenCalledWith(TO_MARTA);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it("leaves out a draft that is already on its way", () => {
    show([REPLY_WRITTEN, { ...TO_MARTA, state: "scheduled" }, BLANK]);
    expect(screen.queryByText("Figures for the board")).toBeNull();
    expect(screen.getByText("Maya Okafor")).toBeVisible();
  });

  // Sends scheduled for later (campbooks-cloud#392).
  const later = (hours: number): Draft => ({
    ...TO_MARTA,
    state: "scheduled",
    send_at: new Date(NOW.getTime() + hours * 3_600_000).toISOString(),
  });

  it("lists sends scheduled for later above the drafts, soonest first", () => {
    const soon = { ...later(2), id: "0192f1a0-0000-7000-8000-0000000001a1", subject: "Soon" };
    const tuesday = {
      ...later(40),
      id: "0192f1a0-0000-7000-8000-0000000001a2",
      subject: "Tuesday",
    };
    show([tuesday, REPLY_WRITTEN, soon]);
    const group = screen.getByRole("region", { name: "Scheduled" });
    const subjects = [...group.querySelectorAll("[id^=scheduled-]")].map((row) => row.textContent);
    expect(subjects[0]).toContain("Soon");
    expect(subjects[1]).toContain("Tuesday");
    expect(screen.getAllByText(/^Leaves /u)).toHaveLength(2);
    expect(screen.getByText("Maya Okafor")).toBeVisible();
  });

  it("leaves out a send still in its undo window", () => {
    show([
      { ...TO_MARTA, state: "scheduled", send_at: new Date(NOW.getTime() + 10_000).toISOString() },
    ]);
    expect(screen.queryByRole("region", { name: "Scheduled" })).toBeNull();
  });

  it("cancels a scheduled send, and asks before editing one", () => {
    const onCancelSend = vi.fn();
    const onCancelAndEdit = vi.fn();
    const scheduled = later(20);
    show([scheduled], { onCancelSend, onCancelAndEdit });
    fireEvent.click(screen.getByRole("button", { name: "Cancel send: Figures for the board" }));
    expect(onCancelSend).toHaveBeenCalledWith(scheduled);
    // Opening the row takes nothing back on its own.
    fireEvent.click(screen.getByRole("button", { name: /Marta Ferreira/u }));
    expect(onCancelAndEdit).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Keep it scheduled" }));
    expect(screen.queryByRole("button", { name: "Cancel send and edit" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Marta Ferreira/u }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel send and edit" }));
    expect(onCancelAndEdit).toHaveBeenCalledWith(scheduled);
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
