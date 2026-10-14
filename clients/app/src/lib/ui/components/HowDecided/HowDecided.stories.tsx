import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";

import { HowDecided } from "./HowDecided";

const meta = {
  title: "Components/HowDecided",
  component: HowDecided,
  args: {
    rows: [
      {
        question: "Needs a reply",
        answer: "Yes. It asks you to reply (“could you confirm”).",
        source: "Rules · 0.90 · acts from 0.88",
      },
      {
        question: "Mentions a date",
        answer: "Yes (“Wednesday”), read as Wed 14 Oct.",
        source: "Rules · 0.96 · acts from 0.75",
      },
      {
        question: "Looks suspicious",
        answer: "No. Nothing in it looks like phishing.",
        source: "Rules · 0.99 · a suggestion",
      },
    ],
    footnote:
      "Rules answer from what the email shows on its face. An answer acts only from the confidence its eval holds at.",
  },
} satisfies Meta<typeof HowDecided>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Closed by default: the numbers stay out of the way. */
export const Closed: Story = {};

/** Opened: the answers in words, then where the numbers come from. */
export const Open: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByText("How Campbooks decided"));
    await expect(canvas.getByText("Rules · 0.90 · acts from 0.88")).toBeVisible();
  },
};

/** No answers: nothing to show. */
export const Empty: Story = { args: { rows: [] } };
