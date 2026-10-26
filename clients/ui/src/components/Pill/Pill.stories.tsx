import type { Meta, StoryObj } from "@storybook/react-vite";

import { SIGNALS } from "../../tokens/tokens";

import { Pill } from "./Pill";

const meta = {
  title: "Components/Pill",
  component: Pill,
  args: { signal: "reply" },
  argTypes: { signal: { control: "select", options: SIGNALS } },
} satisfies Meta<typeof Pill>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Signals carry meaning only: what the item asks of you. */
export const AllSignals: Story = {
  render: () => (
    <div className="flex flex-wrap gap-2">
      {SIGNALS.map((signal) => (
        <Pill key={signal} signal={signal} />
      ))}
    </div>
  ),
};

export const CustomLabel: Story = { args: { signal: "pay", children: "Due Friday" } };
