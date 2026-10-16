import type { Meta, StoryObj } from "@storybook/react-vite";

import { StatusText } from "./StatusText";

const meta = {
  title: "Components/StatusText",
  component: StatusText,
  args: { tone: "ok", children: "Syncing" },
} satisfies Meta<typeof StatusText>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ok: Story = {};
export const Waiting: Story = {
  args: { tone: "waiting", children: "Paused by Gmail for a few minutes" },
};
export const Problem: Story = { args: { tone: "problem", children: "Needs you to sign in again" } };

/** Long words wrap inside a narrow column. */
export const Narrow: Story = {
  render: () => (
    <div className="flex w-sidebar">
      <StatusText tone="waiting">Não está a sincronizar — vamos continuar a tentar</StatusText>
    </div>
  ),
};
