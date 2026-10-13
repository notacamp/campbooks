import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { WorkspaceMenu } from "./WorkspaceMenu";

const WORKSPACES = [
  { id: "w1", name: "Duarte & Pessoa", mark: "DP", kind: "Business", accent: "business" },
  { id: "w2", name: "Household", mark: "HO", kind: "Personal", accent: "personal" },
] as const;

describe("WorkspaceMenu", () => {
  it("lists workspaces, marks the current one and opens another", () => {
    const onSelect = vi.fn();
    const onClose = vi.fn();
    renderUi(
      <WorkspaceMenu
        open
        onClose={onClose}
        workspaces={WORKSPACES}
        currentId="w1"
        onSelect={onSelect}
      />,
    );

    expect(screen.getByRole("dialog", { name: "Switch workspace" })).toBeInTheDocument();
    const current = screen.getByRole("button", { name: /Duarte & Pessoa/u });
    expect(current).toHaveAttribute("aria-current", "true");
    expect(current).toHaveFocus();
    expect(screen.queryByRole("button", { name: /Create a workspace/u })).toBeNull();

    fireEvent.keyDown(current, { key: "ArrowDown" });
    expect(screen.getByRole("button", { name: /Household/u })).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("button", { name: /Household/u }), { key: "ArrowUp" });
    expect(current).toHaveFocus();
    fireEvent.keyDown(current, { key: "x" });

    fireEvent.click(screen.getByRole("button", { name: /Household/u }));
    expect(onClose).toHaveBeenCalled();
    expect(onSelect).toHaveBeenCalledWith("w2");
  });

  it("offers to create a workspace", () => {
    const onCreate = vi.fn();
    renderUi(
      <WorkspaceMenu
        open
        onClose={vi.fn()}
        workspaces={WORKSPACES}
        onSelect={vi.fn()}
        onCreate={onCreate}
        createShortcut="workspace.switch"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Create a workspace/u }));

    expect(onCreate).toHaveBeenCalledOnce();
  });
});
