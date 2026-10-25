import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { BlockHandle } from "./BlockHandle";

const meta = {
  title: "Components/BlockHandle",
  component: BlockHandle,
  args: {
    top: 0,
    addLabel: "Add a block below",
    menuLabel: "Drag to move, click for the block menu",
    onAdd: fn(),
    onMenu: fn(),
    onDragStart: fn(),
  },
  decorators: [
    (Story) => (
      <div className="relative ml-12 w-full max-w-measure">
        <p>A block of text the handle sits beside.</p>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof BlockHandle>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Add a block below" }));
    await expect(args.onAdd).toHaveBeenCalledTimes(1);
    await userEvent.click(
      canvas.getByRole("button", { name: "Drag to move, click for the block menu" }),
    );
    await expect(args.onMenu).toHaveBeenCalledTimes(1);
  },
};
