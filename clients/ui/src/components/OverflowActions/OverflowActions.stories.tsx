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

const CHOICES: OverflowAction[] = [
  {
    id: "other",
    label: "Something else…",
    icon: <Visor size="xs" />,
    shortcut: "inbox.noticed.other",
    onSelect: fn(),
  },
  {
    id: "later",
    label: "Not now",
    icon: "clock",
    shortcut: "inbox.noticed.not_now",
    onSelect: fn(),
  },
];

const SECONDARY: OverflowAction[] = [
  {
    id: "never",
    label: "Don’t suggest this",
    icon: "close",
    shortcut: "inbox.noticed.never",
    onSelect: fn(),
  },
  {
    id: "ask",
    label: "Ask Scout",
    icon: <Visor size="xs" />,
    shortcut: "inbox.noticed.ask",
    onSelect: fn(),
  },
];

/**
 * Labelled buttons while they fit, and the rarer actions always in More
 * (Scout noticed's row).
 */
export const Buttons: Story = {
  args: { actions: CHOICES, more: SECONDARY, end: [], buttons: true },
  render: (args) => (
    <div className="flex items-center gap-2">
      <Button shortcut="inbox.noticed.apply">Archive them…</Button>
      <OverflowActions {...args} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: "Not now" })).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "Ask Scout" })).toBeNull();
    await userEvent.click(canvas.getByRole("button", { name: "More actions" }));
    const menu = canvas.getByRole("dialog", { name: "More actions" });
    await expect(within(menu).getByRole("button", { name: /Ask Scout/u })).toBeVisible();
  },
};

/** The same row on a phone: the buttons fold into More with the rest. */
export const ButtonsFolded: Story = {
  args: { actions: CHOICES, more: SECONDARY, end: [], buttons: true },
  render: (args) => (
    <div className="flex w-sidebar items-center gap-2">
      <Button shortcut="inbox.noticed.apply">File under Updates…</Button>
      <OverflowActions {...args} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "More actions" }));
    const menu = canvas.getByRole("dialog", { name: "More actions" });
    await expect(within(menu).getByRole("button", { name: /Not now/u })).toBeVisible();
    await expect(within(menu).getByRole("button", { name: /Don’t suggest this/u })).toBeVisible();
  },
};
