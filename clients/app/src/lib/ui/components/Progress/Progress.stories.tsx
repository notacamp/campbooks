import type { Meta, StoryObj } from "@storybook/react-vite";

import { Progress } from "./Progress";

const meta = {
  title: "Components/Progress",
  component: Progress,
  args: { label: "Moving 1,000 to Updates", value: 340, max: 1000, children: "340 of 1,000 done" },
} satisfies Meta<typeof Progress>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Partway: Story = {};

export const Starting: Story = { args: { value: 0, children: "Starting" } };

export const Finished: Story = { args: { value: 1000, children: "1,000 of 1,000 done" } };
