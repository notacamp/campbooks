import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { InlineConfirm } from "./InlineConfirm";

const meta = {
  title: "Components/InlineConfirm",
  component: InlineConfirm,
  args: {
    children: "Remove Sofia Pessoa from Duarte & Pessoa? She loses access right away.",
    confirmLabel: "Remove",
    cancelLabel: "Cancel",
    onConfirm: fn(),
    onCancel: fn(),
  },
} satisfies Meta<typeof InlineConfirm>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Asking: Story = {};

export const Busy: Story = { args: { busy: true } };
