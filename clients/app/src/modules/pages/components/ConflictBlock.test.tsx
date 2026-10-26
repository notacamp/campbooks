import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "../../../../test/render";

import { ConflictBlock } from "./ConflictBlock";

describe("ConflictBlock", () => {
  it("shows Notion’s version with the three choices", async () => {
    let finish: (settled: boolean) => void = () => {};
    const onChoose = vi.fn(
      () =>
        new Promise<boolean>((resolve) => {
          finish = resolve;
        }),
    );
    renderWithProviders(
      <ConflictBlock removed={false} onChoose={onChoose}>
        <p>We launch on the 21st.</p>
      </ConflictBlock>,
    );

    expect(screen.getByText("Changed in Notion, while you were editing it here")).toBeVisible();
    expect(screen.getByText("Notion’s version")).toBeVisible();
    expect(screen.getByText("We launch on the 21st.")).toBeVisible();
    expect(screen.getByText(/Changes to different paragraphs merge on their own/u)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Keep both" }));
    await waitFor(() => {
      expect(onChoose).toHaveBeenCalledWith("both");
    });
    // The other choices wait while one is being applied.
    expect(screen.getByRole("button", { name: "Keep mine" })).toBeDisabled();
    finish(true);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Keep mine" })).toBeEnabled();
    });
    fireEvent.click(screen.getByRole("button", { name: "Use Notion’s" }));
    fireEvent.click(screen.getByRole("button", { name: "Keep mine" }));
    expect(onChoose).toHaveBeenCalledWith("notion");
  });

  it("says Notion removed the block, with nothing of theirs to show", () => {
    renderWithProviders(<ConflictBlock removed onChoose={() => Promise.resolve(true)} />);
    expect(screen.getByText("Removed in Notion, while you were editing it here")).toBeVisible();
    expect(screen.getByText(/Notion no longer has this block/u)).toBeVisible();
    expect(screen.queryByText("Notion’s version")).toBeNull();
  });

  it("shows a reader the conflict without the choices", () => {
    renderWithProviders(
      <ConflictBlock removed={false} readOnly onChoose={() => Promise.resolve(true)}>
        <p>Theirs</p>
      </ConflictBlock>,
    );
    expect(screen.getByText("Theirs")).toBeVisible();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
