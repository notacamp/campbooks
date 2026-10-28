import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "../../../../../test/render";

import { ColumnConflicts } from "./ColumnConflicts";
import type { TableConflict } from "./model";

const conflict = (ours: unknown, theirs: unknown): TableConflict => ({
  id: "c1",
  kind: "column",
  property: "p1",
  row: null,
  ours,
  theirs,
  ours_at: "2026-10-27T10:30:00Z",
  theirs_at: "2026-10-27T10:42:00Z",
});

describe("ColumnConflicts", () => {
  it("says what differs: the name, the type and how many options", () => {
    renderWithProviders(
      <ColumnConflicts
        conflicts={[
          conflict(
            { name: "Stage", type: "select", config: { options: [{}, {}] } },
            { name: "Phase", type: "multi_select", config: { options: [{}, {}, {}] } },
          ),
        ]}
      />,
    );
    const banner = screen.getByRole("region", { name: "Columns changed in both places" });
    expect(within(banner).getByText("Name: “Stage” here, “Phase” in Notion")).toBeVisible();
    expect(within(banner).getByText("Type: Select here, Multi-select in Notion")).toBeVisible();
    expect(within(banner).getByText("Options: 2 here, 3 in Notion")).toBeVisible();
    // Someone who cannot change the table sees what differs and has no button to press.
    expect(within(banner).queryByRole("button")).toBeNull();
  });

  it("asks for a plain choice when only the options differ", async () => {
    const onChoose = vi.fn(() => Promise.resolve(true));
    renderWithProviders(
      <ColumnConflicts
        conflicts={[
          conflict(
            { name: "Stage", type: "select", config: { options: [{}] } },
            { name: "Stage", type: "select", config: { options: [{}, {}] } },
          ),
        ]}
        onChoose={onChoose}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Keep mine" }));
    await waitFor(() => {
      expect(onChoose).toHaveBeenCalledWith("c1", "ours");
    });
    fireEvent.click(screen.getByRole("button", { name: "Use Notion’s" }));
    await waitFor(() => {
      expect(onChoose).toHaveBeenCalledWith("c1", "theirs");
    });
  });

  it("cuts a long name on its button, not in the sentence", () => {
    const long = "A column name that goes on and on and on";
    renderWithProviders(
      <ColumnConflicts
        conflicts={[conflict({ name: long, type: "text" }, { name: "Short", type: "text" })]}
        onChoose={vi.fn(() => Promise.resolve(true))}
      />,
    );
    expect(screen.getByRole("button", { name: `Keep “${long.slice(0, 31)}…”` })).toBeVisible();
    expect(screen.getByText(new RegExp(`“${long}” was changed here`, "u"))).toBeVisible();
  });

  it("is nothing when no column changed", () => {
    renderWithProviders(<ColumnConflicts conflicts={[]} />);
    expect(screen.queryByRole("region")).toBeNull();
  });
});
