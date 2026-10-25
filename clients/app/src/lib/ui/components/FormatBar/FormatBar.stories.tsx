import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { IconButton } from "@notacamp/ui";

import { FormatBar } from "./FormatBar";

const meta = {
  title: "Components/FormatBar",
  component: FormatBar,
  args: {
    label: "Format",
    always: true,
    items: [
      { id: "block", label: "Insert a block", glyph: "/", onPress: fn() },
      { id: "mention", label: "Mention", glyph: "@", onPress: fn() },
      { id: "todo", label: "Turn into a to-do", icon: "check", onPress: fn() },
      { id: "bold", label: "Bold", icon: "bold", pressed: true, onPress: fn() },
      { id: "italic", label: "Italic", icon: "italic", pressed: false, onPress: fn() },
    ],
    end: <IconButton icon="chevron-down" label="Hide keyboard" size="lg" onClick={fn()} />,
  },
  decorators: [
    (Story) => (
      <div className="w-full max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof FormatBar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("toolbar", { name: "Format" })).toBeInTheDocument();
    await expect(canvas.getByRole("button", { name: "Bold" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await userEvent.click(canvas.getByRole("button", { name: "Mention" }));
  },
};
