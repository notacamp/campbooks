import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { ItemActions, type ItemAction } from "./ItemActions";

const UNIVERSAL: ItemAction[] = [
  { id: "done", label: "Done", key: "item.done" },
  { id: "snooze", label: "Snooze…", key: "item.snooze" },
  { id: "not_needed", label: "Not needed", key: "conversations.resolve_need" },
];

const meta = {
  title: "Components/ItemActions",
  component: ItemActions,
  args: {
    label: "Actions for Marta Ferreira",
    own: [{ id: "reply", label: "Reply", key: "item.choice1" }],
    universal: UNIVERSAL,
    onAction: fn(),
    onHighlight: fn(),
  },
} satisfies Meta<typeof ItemActions>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A need: Reply, then the universal actions after the divider. */
export const Default: Story = {
  play: async ({ canvasElement, args }) => {
    await userEvent.click(within(canvasElement).getByRole("button", { name: /Done/ }));
    await expect(args.onAction).toHaveBeenCalledWith("done");
  },
};

/** A question: Yes and No take 1 and 2. */
export const Question: Story = {
  args: {
    own: [
      { id: "yes", label: "Yes", key: "item.choice1" },
      { id: "no", label: "No", key: "item.choice2" },
    ],
    universal: [{ id: "snooze", label: "Snooze…", key: "item.snooze" }],
  },
};

/** Three own actions, one running the AI (the visor before its label). */
export const ThreeWithAi: Story = {
  args: {
    own: [
      { id: "yescal", label: "Say yes and add to calendar", key: "item.choice1", ai: true },
      { id: "draft", label: "Draft a reply", key: "item.choice2", ai: true },
      { id: "cal", label: "Only add to calendar", key: "item.choice3" },
    ],
  },
};

/** The arrow keys' highlight: a visible ring, and the focus. */
export const Highlighted: Story = { args: { highlighted: "snooze" } };

function Walk() {
  const [highlighted, setHighlighted] = useState<string | undefined>("reply");
  return (
    <ItemActions
      label="Actions"
      own={[{ id: "reply", label: "Reply", key: "item.choice1" }]}
      universal={UNIVERSAL}
      highlighted={highlighted}
      onHighlight={setHighlighted}
      onAction={fn()}
    />
  );
}

/** Tab moves the highlight with the focus. */
export const TabMoves: StoryObj = {
  render: () => <Walk />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: /Reply/ })).toHaveFocus();
    canvas.getByRole("button", { name: /Snooze/ }).focus();
    await expect(canvas.getByRole("button", { name: /Snooze/ })).toHaveAttribute(
      "data-highlighted",
      "true",
    );
  },
};
