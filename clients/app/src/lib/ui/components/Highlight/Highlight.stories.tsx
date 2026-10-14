import type { Meta, StoryObj } from "@storybook/react-vite";

import { Highlight } from "./Highlight";

const meta = {
  title: "Components/Highlight",
  component: Highlight,
  args: {
    segments: [
      { text: "Invoice ", hit: false },
      { text: "October", hit: true },
      { text: " for the studio", hit: false },
    ],
  },
} satisfies Meta<typeof Highlight>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Several matches, and one at the very start. */
export const Several: Story = {
  args: {
    segments: [
      { text: "Fatura", hit: true },
      { text: " de ", hit: false },
      { text: "outubro", hit: true },
      { text: ": a ", hit: false },
      { text: "fatura", hit: true },
      { text: " segue em anexo", hit: false },
    ],
  },
};

/** Text that looks like markup stays text. */
export const NotHtml: Story = {
  args: { segments: [{ text: "<b>bold</b> & <img src=x>", hit: true }] },
};
