import type { Meta, StoryObj } from "@storybook/react-vite";

import { Checkbox } from "./Checkbox";

const meta = {
  title: "Components/Checkbox",
  component: Checkbox,
  args: {
    label: "Also use this account to sign in",
    hint: "Untick it for a shared mailbox, like support@.",
    defaultChecked: true,
  },
} satisfies Meta<typeof Checkbox>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ticked: Story = {};
export const Unticked: Story = { args: { defaultChecked: false } };
export const WithoutHint: Story = { args: { hint: undefined } };
export const Disabled: Story = { args: { disabled: true } };
