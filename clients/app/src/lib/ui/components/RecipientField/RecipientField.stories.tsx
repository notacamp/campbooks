import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { RecipientField, type Recipient } from "./RecipientField";

function Controlled({ initial = [] }: { initial?: Recipient[] }) {
  const [recipients, setRecipients] = useState<Recipient[]>(initial);
  return <RecipientField label="To" recipients={recipients} onChange={setRecipients} />;
}

const meta = {
  title: "Components/RecipientField",
  component: Controlled,
  decorators: [
    (Story) => (
      <div className="max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Controlled>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {};

export const WithRecipients: Story = {
  args: {
    initial: [
      { name: "Maya Okafor", address: "maya@northwind-studio.example" },
      { address: "leo@example.com" },
    ],
  },
};

/** Enter, a comma or Tab turns what you typed into a chip; Backspace removes the last one. */
export const AddWithTheKeyboard: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("textbox", { name: "To" });
    await userEvent.type(input, "ana@example.com{Enter}Leo Park <leo@example.com>,");
    await expect(canvas.getByText("ana@example.com")).toBeVisible();
    await expect(canvas.getByText("Leo Park")).toBeVisible();
    await userEvent.type(input, "{Backspace}");
    await expect(canvas.queryByText("Leo Park")).not.toBeInTheDocument();
  },
};

export const InvalidAddress: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("textbox", { name: "To" });
    await userEvent.type(input, "not an address{Enter}");
    await expect(input).toHaveAttribute("aria-invalid", "true");
    await expect(canvas.getByText(/This is not an email address/u)).toBeVisible();
  },
};
