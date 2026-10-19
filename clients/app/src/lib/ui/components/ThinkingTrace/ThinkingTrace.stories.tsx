import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";

import { ThinkingTrace } from "./ThinkingTrace";

const THOUGHT =
  "Marta asks whether Thursday at 10:00 works for the site visit.\n" +
  "Sam wrote to her before and signs with his first name.\n" +
  "The contractor needs an answer by Wednesday, so say yes clearly.\n" +
  "Keep it short and warm, in Portuguese, as she wrote.\n" +
  "Confirm the time and the place.\n" +
  "End with Sam's name.";

const meta = {
  title: "Components/ThinkingTrace",
  component: ThinkingTrace,
  args: {
    label: "Scout is thinking…",
    text: THOUGHT,
    live: true,
    summary: "How Scout thought about it",
  },
  parameters: { layout: "padded" },
} satisfies Meta<typeof ThinkingTrace>;

export default meta;
type Story = StoryObj<typeof meta>;

/** While Scout thinks: the newest lines, under a status that is announced. */
export const Thinking: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("status")).toHaveTextContent("Scout is thinking…");
    await expect(canvas.getByText(/End with Sam's name/u)).toBeInTheDocument();
  },
};

/** Before the first thought arrives: the status alone. */
export const Starting: Story = { args: { text: "" } };

/** Once the reply is written: folded away, one click from the thinking. */
export const Thought: Story = {
  args: { live: false },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByText("How Scout thought about it"));
    await expect(canvas.getByText(/End with Sam's name/u)).toBeVisible();
  },
};
