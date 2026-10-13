import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";

import { CodeBlock } from "./CodeBlock";

const meta = {
  title: "Components/CodeBlock",
  component: CodeBlock,
  parameters: { layout: "padded" },
  args: {
    label: "Last error",
    children: "provider returned 503: upstream timed out while syncing the mailbox",
    copyLabel: "Copy error",
    copiedLabel: "Copied.",
    failedLabel: "Couldn’t copy. Select the text and copy it.",
    copy: () => Promise.resolve(),
  },
  decorators: [
    (Story) => (
      <div className="max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof CodeBlock>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Short: Story = {};

/** Long unbroken text wraps instead of scrolling sideways. */
export const Long: Story = {
  args: {
    children: `${"error: connection reset by peer while reading https://example.com/".repeat(3)}\n${"  at step ".repeat(60)}`,
  },
};

/** Copying says so. */
export const Copied: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Copy error" }));
    await expect(await canvas.findByText("Copied.")).toBeVisible();
  },
};

/** When the clipboard refuses, it says so. */
export const ClipboardRefused: Story = {
  args: { copy: () => Promise.reject(new Error("denied")) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Copy error" }));
    await expect(
      await canvas.findByText("Couldn’t copy. Select the text and copy it."),
    ).toBeVisible();
  },
};
