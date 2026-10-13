import type { Meta, StoryObj } from "@storybook/react-vite";

import { SIGNALS } from "../../tokens/tokens";

import { Dot } from "./Dot";

const meta = {
  title: "Components/Dot",
  component: Dot,
} satisfies Meta<typeof Dot>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Neutral: Story = {};

export const AllSignals: Story = {
  render: () => (
    <ul className="flex flex-col gap-2">
      {SIGNALS.map((signal) => (
        <li key={signal} className="flex items-center gap-2">
          <Dot signal={signal} />
          {signal}
        </li>
      ))}
    </ul>
  ),
};
