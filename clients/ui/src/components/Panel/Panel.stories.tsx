import type { Meta, StoryObj } from "@storybook/react-vite";

import { Panel } from "./Panel";

const meta = {
  title: "Components/Panel",
  component: Panel,
  args: { label: "Changed in Notion", children: "Notion's version of this paragraph." },
} satisfies Meta<typeof Panel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Solid: Story = {};

export const Dashed: Story = {
  args: { variant: "dashed", row: true, label: "Kept in Notion", children: "Columns (2)" },
};
