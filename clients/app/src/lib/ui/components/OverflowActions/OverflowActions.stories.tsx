import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";
import { Visor } from "../Visor/Visor";

import { OverflowActions, type OverflowAction } from "./OverflowActions";

const ACTIONS: OverflowAction[] = [
  { id: "trash", label: "Delete", icon: "trash", shortcut: "conversations.trash", onSelect: fn() },
  {
    id: "spam",
    label: "Report spam",
    icon: "warning",
    shortcut: "conversations.report_spam",
    onSelect: fn(),
  },
  { id: "pin", label: "Pin", icon: "pin", shortcut: "conversations.star", onSelect: fn() },
  {
    id: "unread",
    label: "Mark unread",
    icon: "unread",
    shortcut: "conversations.mark_unread",
    onSelect: fn(),
  },
];

const END: OverflowAction[] = [
  {
    id: "previous",
    label: "Previous",
    icon: "chevron-up",
    shortcut: "list.previous",
    onSelect: fn(),
  },
  { id: "next", label: "Next", icon: "chevron-down", shortcut: "list.next", onSelect: fn() },
];

const meta = {
  title: "Components/OverflowActions",
  component: OverflowActions,
  args: { label: "More actions", actions: ACTIONS, end: END },
  // A toolbar row: a labelled action first, the overflow in the rest.
  render: (args) => (
    <div className="flex items-center gap-2">
      <Button icon="archive" shortcut="item.done">
        Done
      </Button>
      <OverflowActions {...args} />
    </div>
  ),
} satisfies Meta<typeof OverflowActions>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Room for every icon: the actions, then Previous and Next at the end. */
export const Fits: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: "Delete" })).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "More actions" })).toBeNull();
  },
};

/** A narrow row: everything folds into More, each row with its name and key. */
export const Folded: Story = {
  render: (args) => (
    <div className="flex w-sidebar items-center gap-2">
      <Button icon="archive" shortcut="item.done">
        Done
      </Button>
      <Button icon="reply" shortcut="item.reply">
        Reply
      </Button>
      <OverflowActions {...args} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "More actions" }));
    const menu = canvas.getByRole("dialog", { name: "More actions" });
    await expect(within(menu).getByRole("button", { name: /Report spam/u })).toBeVisible();
    await expect(within(menu).getByRole("button", { name: /Next/u })).toBeVisible();
  },
};

/** An action that asks Scout: the visor stands for its icon. */
export const WithScout: Story = {
  args: {
    actions: [
      ...ACTIONS,
      { id: "scout", label: "Ask Scout", icon: <Visor size="xs" />, onSelect: fn() },
    ],
  },
};
