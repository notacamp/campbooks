import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";
import { NavItem } from "../NavItem/NavItem";

import { ConversationRow } from "./ConversationRow";

describe("ConversationRow", () => {
  it("is a button marking the open conversation", () => {
    const onSelect = vi.fn();
    renderUi(
      <ConversationRow
        from="Cloudhost"
        avatar={{ kind: "service" }}
        time="12 Oct"
        subject="Invoice"
        snippet="Due Friday."
        unread
        current
        onSelect={onSelect}
      />,
    );
    const row = screen.getByRole("button", { name: /Cloudhost/ });

    fireEvent.click(row);

    expect(onSelect).toHaveBeenCalledOnce();
    expect(row).toHaveAttribute("aria-current", "true");
    expect(row).toHaveAttribute("data-unread", "true");
  });

  it("leaves aria-current off the others", () => {
    renderUi(
      <ConversationRow
        from="Rui Matos"
        time="Mon"
        subject="VAT"
        snippet="Receipts"
        onSelect={vi.fn()}
      />,
    );

    expect(screen.getByRole("button")).not.toHaveAttribute("aria-current");
  });
});

describe("NavItem", () => {
  it("marks the current page and exposes its single-key shortcut", () => {
    renderUi(<NavItem label="Primary" count={12} keys="1" current sub />);
    const item = screen.getByRole("button", { name: /Primary/ });

    expect(item).toHaveAttribute("aria-current", "page");
    expect(item).toHaveAttribute("aria-keyshortcuts", "1");
    expect(item).toHaveTextContent("Primary 121");
  });

  it("is a link with an href and hints its sequence", () => {
    const onClick = vi.fn();
    renderUi(
      <NavItem label="Inbox" icon="inbox" shortcut="nav.inbox" href="#inbox" onClick={onClick} />,
    );
    const link = screen.getByRole("link", { name: /Inbox/ });

    fireEvent.click(link);

    expect(onClick).toHaveBeenCalledOnce();
    expect(link).not.toHaveAttribute("aria-current");
    expect(link).toHaveTextContent("Inbox GI");
  });
});
