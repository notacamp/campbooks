import type { Meta, StoryObj } from "@storybook/react-vite";

import { Button } from "../Button/Button";

import { EmptyState } from "./EmptyState";

const meta = {
  title: "Components/EmptyState",
  component: EmptyState,
  args: {
    title: "Nothing needs you",
    children: "Campbooks keeps reading and will bring things here when they do.",
  },
} satisfies Meta<typeof EmptyState>;

export default meta;
type Story = StoryObj<typeof meta>;

export const AllClear: Story = {};

export const EmptyStream: Story = {
  args: {
    title: "Nothing in Clients",
    children: "Mail that matches this stream’s rule will land here.",
    visor: "idle",
    visorSize: "lg",
    action: <Button>Edit the rule</Button>,
  },
};

export const FailedToLoad: Story = {
  args: {
    title: "Could not open this conversation",
    children: "Google did not answer in time. Nothing was lost.",
    visor: "thinking",
    visorSize: "lg",
    action: <Button shortcut="item.reply">Try again</Button>,
  },
};
