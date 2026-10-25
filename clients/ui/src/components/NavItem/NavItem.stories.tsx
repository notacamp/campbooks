import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";

import { NavDivider, NavItem } from "./NavItem";

const meta = {
  title: "Components/NavItem",
  component: NavItem,
  args: { label: "Inbox", icon: "inbox", count: 7, shortcut: "nav.inbox" },
  decorators: [
    (Story) => (
      <div className="flex w-sidebar flex-col">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NavItem>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Shortcut hints show while the sidebar is hovered or the item has focus. */
export const Default: Story = {};

export const Current: Story = {
  args: { label: "Home", icon: "home", count: 6, shortcut: "nav.home", current: true },
  play: async ({ canvasElement }) => {
    const item = within(canvasElement).getByRole("button", { name: /Home/ });
    await expect(item).toHaveAttribute("aria-current", "page");
  },
};

/** A stream under its place. */
export const Sub: Story = {
  args: { label: "Primary", icon: undefined, count: 12, sub: true, shortcut: undefined, keys: "1" },
};

export const AsLink: Story = {
  args: {
    label: "Documents",
    icon: "document",
    count: undefined,
    shortcut: "nav.documents",
    href: "#documents",
  },
};

/** The streams, a quiet divider, then the views beside them, each with its icon. */
export const StreamsThenViews: Story = {
  render: () => (
    <>
      <NavItem label="Inbox" icon="inbox" shortcut="nav.inbox" />
      <NavItem label="Primary" sub count={12} keys="g 1" current />
      <NavItem label="Updates" sub keys="g 2" />
      <NavDivider />
      <NavItem label="Sent" icon="send" sub keys="g 5" />
      <NavItem label="Archive" icon="archive" sub keys="g 6" />
      <NavItem label="Spam" icon="warning" sub keys="g 7" />
      <NavItem label="Trash" icon="trash" sub keys="g 8" />
    </>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("separator")).toBeInTheDocument();
    await expect(canvas.getByRole("button", { name: /Trash/ })).toBeInTheDocument();
  },
};
