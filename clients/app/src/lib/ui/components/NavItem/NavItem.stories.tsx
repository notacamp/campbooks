import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";

import { NavItem } from "./NavItem";

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

/** Sent, set a little apart from the streams above it. */
export const Apart: Story = {
  args: {
    label: "Sent",
    icon: undefined,
    count: undefined,
    sub: true,
    apart: true,
    shortcut: undefined,
    keys: "g 5",
  },
  play: async ({ canvasElement }) => {
    const item = within(canvasElement).getByRole("button", { name: /Sent/ });
    await expect(item).toHaveAttribute("data-apart", "true");
  },
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
