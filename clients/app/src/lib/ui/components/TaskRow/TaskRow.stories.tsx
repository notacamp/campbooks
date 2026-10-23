import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { TaskRow } from "./TaskRow";

const meta = {
  title: "Components/TaskRow",
  component: TaskRow,
  args: {
    title: "Send the signed lease to Ana",
    kind: "Do",
    meta: "From Ana Ribeiro · email",
    chips: [{ text: "Due today" }],
    checkLabel: "Mark done: Send the signed lease to Ana",
    onOpen: fn(),
    onCheck: fn(),
  },
  decorators: [
    (Story) => (
      <div className="flex max-w-measure flex-col">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof TaskRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** A bill that is late: the date is drawn in the danger colour. */
export const Overdue: Story = {
  args: {
    title: "EDP electricity bill · €84.20",
    kind: "Pay",
    meta: "From EDP · email",
    chips: [{ text: "Overdue · 2 days", tone: "danger" }],
    checkLabel: "Mark paid: EDP electricity bill",
  },
};

/** One the member wrote: it says so, and has a plan as well as a date. */
export const OwnWithPlan: Story = {
  args: {
    title: "Call the accountant about Q3 VAT",
    meta: "You wrote this",
    chips: [{ text: "Due Fri 30" }, { text: "Planned Wed 28" }],
    checkLabel: "Mark done: Call the accountant about Q3 VAT",
  },
};

/** A task someone else holds has no check: only its assignee can close it. */
export const WithSomeoneElse: Story = {
  args: { meta: "You wrote this", chips: [{ text: "With João" }], onCheck: undefined },
};

export const Selected: Story = { args: { selected: true } };
export const Open: Story = { args: { current: true } };

/** A long title is cut on one line. */
export const LongTitle: Story = {
  args: {
    title:
      "Send the signed lease for Rua das Flores 12 to Ana Ribeiro together with the three attachments she asked for",
  },
};

/** The check and the row are separate buttons. */
export const Interaction: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: /Mark done/u }));
    await expect(args.onCheck).toHaveBeenCalledTimes(1);
    await userEvent.click(canvas.getByRole("button", { name: /Send the signed lease/u }));
    await expect(args.onOpen).toHaveBeenCalledTimes(1);
  },
};
