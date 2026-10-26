import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";

import { Disclosure } from "./Disclosure";

const meta = {
  title: "Components/Disclosure",
  component: Disclosure,
  args: {
    summary: "What Campbooks reads and keeps",
    children: (
      <>
        <p>Campbooks keeps your name, your address and the permission Google gives it.</p>
        <p>Nothing is connected to a workspace until you say so.</p>
      </>
    ),
  },
} satisfies Meta<typeof Disclosure>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Closed: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByText("What Campbooks reads and keeps"));
    await expect(canvas.getByText(/Nothing is connected/u)).toBeVisible();
  },
};

export const Open: Story = { args: { defaultOpen: true } };
