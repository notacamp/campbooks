import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";

import { CopyField } from "./CopyField";

const meta = {
  title: "Components/CopyField",
  component: CopyField,
  args: {
    label: "Invitation link for sofia@example.com",
    value: "https://campbooks.example.com/invitations/accept?token=demo-token-not-real",
    copyLabel: "Copy link",
    copiedLabel: "Link copied.",
    failedLabel: "Couldn’t copy. Select the link and copy it.",
    copy: () => Promise.resolve(),
  },
} satisfies Meta<typeof CopyField>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ready: Story = {};

/** Copying says so. */
export const Copied: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Copy link" }));
    await expect(await canvas.findByText("Link copied.")).toBeVisible();
  },
};

/** When the clipboard refuses, the link is selected for the platform's own copy. */
export const ClipboardRefused: Story = {
  args: { copy: () => Promise.reject(new Error("denied")) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Copy link" }));
    await expect(
      await canvas.findByText("Couldn’t copy. Select the link and copy it."),
    ).toBeVisible();
  },
};
