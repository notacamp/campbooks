import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";
import { ActionBar } from "../ActionBar/ActionBar";

import { TabBar } from "./TabBar";

describe("TabBar", () => {
  it("marks the current place and shows a badge", () => {
    const onInbox = vi.fn();
    renderUi(
      <TabBar
        phoneOnly
        items={[
          { id: "home", label: "Home", icon: "home", current: true, badge: 5 },
          { id: "inbox", label: "Inbox", icon: "inbox", onClick: onInbox },
          { id: "money", label: "Money", icon: "money", href: "#money" },
        ]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Inbox" }));

    expect(screen.getByRole("navigation", { name: "Main" })).toHaveAttribute(
      "data-phone-only",
      "true",
    );
    expect(screen.getByRole("button", { name: "Home 5" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Money" })).toHaveAttribute("href", "#money");
    expect(onInbox).toHaveBeenCalledOnce();
  });
});

describe("ActionBar", () => {
  it("groups a screen's main actions under a name", () => {
    renderUi(
      <ActionBar label="Conversation actions">
        <button type="button">Reply</button>
      </ActionBar>,
    );

    expect(screen.getByRole("group", { name: "Conversation actions" })).toContainElement(
      screen.getByRole("button"),
    );
  });
});
