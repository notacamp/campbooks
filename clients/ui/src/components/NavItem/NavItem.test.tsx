import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";

import { NavDivider, NavItem } from "./NavItem";

describe("NavItem and NavDivider", () => {
  it("marks the page you are on, and a divider separates the groups", () => {
    renderUi(
      <nav aria-label="Inbox">
        <NavItem label="Primary" sub href="/inbox/primary" current />
        <NavDivider />
        <NavItem label="Sent" icon="send" sub href="/inbox/sent" />
      </nav>,
    );

    expect(screen.getByRole("link", { name: "Primary" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Sent" })).not.toHaveAttribute("aria-current");
    expect(screen.getByRole("separator")).toBeInTheDocument();
  });
});
