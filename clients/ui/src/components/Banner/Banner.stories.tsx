import type { Meta, StoryObj } from "@storybook/react-vite";

import { Button } from "../Button/Button";

import { Banner } from "./Banner";

const meta = {
  title: "Components/Banner",
  component: Banner,
  args: { title: "You are offline.", children: "2 changes will sync when you reconnect." },
} satisfies Meta<typeof Banner>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Neutral: Story = {};

export const Danger: Story = {
  args: {
    tone: "danger",
    title: "ana@duartepessoa.example stopped syncing.",
    children: "Google asked you to sign in again.",
    action: <Button>Reconnect</Button>,
  },
};
