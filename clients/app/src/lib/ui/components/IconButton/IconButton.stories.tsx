import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { Visor } from "../Visor/Visor";

import { IconButton } from "./IconButton";

const meta = {
  title: "Components/IconButton",
  component: IconButton,
  args: { label: "Next conversation", icon: "chevron-down", shortcut: "list.next", onClick: fn() },
} satisfies Meta<typeof IconButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement, args }) => {
    const button = within(canvasElement).getByRole("button", { name: "Next conversation" });
    await expect(button).toHaveAttribute("aria-keyshortcuts", "J");
    await userEvent.click(button);
    await expect(args.onClick).toHaveBeenCalledOnce();
  },
};

/** 44px, for touch. */
export const Large: Story = {
  args: { size: "lg", label: "Snooze", icon: "clock", shortcut: "item.snooze" },
};

export const WithVisor: Story = {
  args: { size: "lg", label: "Search or ask", icon: <Visor size="md" />, shortcut: "command.open" },
};

export const All: Story = {
  render: () => (
    <div className="flex items-center gap-2">
      <IconButton label="Previous conversation" icon="chevron-up" shortcut="list.previous" />
      <IconButton label="Next conversation" icon="chevron-down" shortcut="list.next" />
      <IconButton label="More actions" icon="more" size="lg" />
      <IconButton label="Keyboard shortcuts" icon="keyboard" shortcut="sheet.open" />
    </div>
  ),
};
