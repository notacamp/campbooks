import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";

import { TabBar } from "./TabBar";

const meta = {
  title: "Components/TabBar",
  component: TabBar,
  parameters: { layout: "fullscreen" },
  args: {
    items: [
      { id: "home", label: "Home", icon: "home", current: true, badge: 5 },
      { id: "inbox", label: "Inbox", icon: "inbox" },
      { id: "schedule", label: "Schedule", icon: "schedule" },
      { id: "documents", label: "Documents", icon: "document" },
      { id: "money", label: "Money", icon: "money" },
    ],
  },
} satisfies Meta<typeof TabBar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const nav = within(canvasElement).getByRole("navigation", { name: "Main" });
    await expect(within(nav).getByRole("button", { name: /Home/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
  },
};

export const Links: Story = {
  args: {
    items: [
      { id: "home", label: "Home", icon: "home", href: "#home" },
      { id: "inbox", label: "Inbox", icon: "inbox", href: "#inbox", current: true },
      { id: "money", label: "Money", icon: "money", href: "#money" },
    ],
  },
};
