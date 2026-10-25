import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { PageTree, PageTreeGroup, PageTreeRow } from "./PageTree";

describe("PageTree", () => {
  it("opens, toggles and adds with three separate buttons", () => {
    const onOpen = vi.fn();
    const onToggle = vi.fn();
    const onAdd = vi.fn();
    renderUi(
      <PageTree label="Pages">
        <PageTreeGroup name="Workspace">
          <PageTreeRow
            label="Team handbook"
            expanded={false}
            toggleLabel="Show sub-pages of Team handbook"
            addLabel="Add a sub-page to Team handbook"
            onOpen={onOpen}
            onToggle={onToggle}
            onAdd={onAdd}
          />
        </PageTreeGroup>
      </PageTree>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Show sub-pages of Team handbook" }));
    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(onOpen).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Team handbook" }));
    expect(onOpen).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Add a sub-page to Team handbook" }));
    expect(onAdd).toHaveBeenCalledTimes(1);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it("marks the page on screen and has no toggle for a leaf", () => {
    renderUi(
      <PageTree label="Pages">
        <PageTreeGroup name="Private">
          <PageTreeRow label="Weekly plan" current onOpen={vi.fn()} />
        </PageTreeGroup>
      </PageTree>,
    );
    expect(screen.getByRole("button", { name: "Weekly plan" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.queryByRole("button", { name: /sub-pages/u })).toBeNull();
    expect(screen.getByRole("navigation", { name: "Pages" })).toBeInTheDocument();
  });

  it("says so when a group is empty", () => {
    renderUi(
      <PageTree label="Pages">
        <PageTreeGroup name="Shared with you" empty="Nothing is shared with you yet." />
      </PageTree>,
    );
    expect(screen.getByText("Nothing is shared with you yet.")).toBeInTheDocument();
  });
});
