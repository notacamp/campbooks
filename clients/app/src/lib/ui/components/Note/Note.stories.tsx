import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";

import { Note } from "./Note";

const meta = {
  title: "Components/Note",
  component: Note,
  args: {
    children:
      "Sofia is waiting on your review of the Q4 proposal by Thursday. Version 3 moves the timeline two weeks; the price is unchanged.",
    links: [
      { icon: "check", title: "Review Q4 proposal", kind: "Task, Thu", onSelect: fn() },
      { icon: "document", title: "Brightloop-Q4-scope-v3.pdf", kind: "Document", onSelect: fn() },
      { icon: "event", title: "Held Thu 09:00", kind: "Schedule", onSelect: fn() },
    ],
    confidence: "Sorted into Primary, 98% sure",
    why: [
      "You have replied to Sofia 14 times this year.",
      "The message asks you a direct question with a deadline.",
      "It is not a bulk or automated sender.",
    ],
    footer: <Button variant="link">Move</Button>,
  },
} satisfies Meta<typeof Note>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** "Why?" is a disclosure: it reveals the reasons and says it is expanded. */
export const WhyOpen: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const why = canvas.getByRole("button", { name: "Why?" });
    await userEvent.click(why);
    await expect(canvas.getByRole("button", { name: "Hide why" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    await expect(canvas.getByText("You have replied to Sofia 14 times this year.")).toBeVisible();
  },
};

/** Below 80% the assistant asks instead of acting. */
export const LowConfidence: Story = {
  args: {
    visor: "unsure",
    children: "An invitation to a workshop. Campbooks has not seen this sender before.",
    links: [],
    choice: {
      prompt: "Where does this go?",
      options: [
        { label: "Primary", keys: "1", onSelect: fn() },
        { label: "Updates", keys: "2", onSelect: fn() },
      ],
    },
    confidence: "61% sure. Below 80% Campbooks asks instead of acting.",
    why: [],
    footer: undefined,
  },
};

/** Phones: smaller text and visor. */
export const Compact: Story = {
  args: {
    compact: true,
    children:
      "Sofia is waiting on your review by Thursday. Version 3 moves the timeline two weeks; the price is unchanged.",
    links: [
      { icon: "check", title: "Review Q4 proposal", onSelect: fn() },
      { icon: "event", title: "Held Thu 09:00", onSelect: fn() },
    ],
    confidence: "Primary, 98% sure",
    footer: undefined,
  },
};
