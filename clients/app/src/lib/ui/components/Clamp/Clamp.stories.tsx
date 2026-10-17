import type { Meta, StoryObj } from "@storybook/react-vite";

import { Clamp } from "./Clamp";

const LONG = Array.from(
  { length: 12 },
  (_, index) =>
    `Paragraph ${String(index + 1)}: the contractor can come to the site this week, and will bring the revised plans.`,
);

const meta = {
  title: "Components/Clamp",
  component: Clamp,
  args: { moreLabel: "Show the whole email", lessLabel: "Show less", children: null },
} satisfies Meta<typeof Clamp>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Long content: a few lines with a fade, and the button to read it all. */
export const Long: Story = {
  render: (args) => (
    <Clamp {...args}>
      {LONG.map((line) => (
        <p key={line}>{line}</p>
      ))}
    </Clamp>
  ),
};

/** Short content: no fade, no button. */
export const Short: Story = {
  render: (args) => (
    <Clamp {...args}>
      <p>Thursday at 10:00 works for me.</p>
    </Clamp>
  ),
};
