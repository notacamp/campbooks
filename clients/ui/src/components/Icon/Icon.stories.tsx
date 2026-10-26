import type { Meta, StoryObj } from "@storybook/react-vite";

import { Icon, ICON_NAMES } from "./Icon";

const meta = {
  title: "Components/Icon",
  component: Icon,
  args: { name: "inbox" },
  argTypes: { name: { control: "select", options: ICON_NAMES } },
} satisfies Meta<typeof Icon>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Large: Story = { args: { size: "lg" } };

export const All: Story = {
  render: () => (
    <ul className="grid grid-cols-2 gap-3 sm:grid-cols-4">
      {ICON_NAMES.map((name) => (
        <li key={name} className="flex items-center gap-2">
          <Icon name={name} size="lg" />
          {name}
        </li>
      ))}
    </ul>
  ),
};
