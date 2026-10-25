import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { TitleField } from "./TitleField";

const meta = {
  title: "Components/TitleField",
  component: TitleField,
  args: { label: "Page title", placeholder: "Untitled", defaultValue: "Client onboarding" },
  decorators: [
    (Story) => (
      <div className="w-full max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof TitleField>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Empty: Story = { args: { defaultValue: "" } };
export const ReadOnly: Story = { args: { readOnly: true } };
export const LongTitle: Story = {
  args: {
    defaultValue:
      "A page title that is far longer than the line it sits on, so it wraps onto as many lines as it needs",
  },
};

/** One long word still wraps inside the box. */
export const LongWord: Story = {
  args: { defaultValue: "Supercalifragilisticexpialidocious-Supercalifragilisticexpialidocious" },
};

export const Typing: Story = {
  args: { defaultValue: "", onChange: fn() },
  play: async ({ canvasElement, args }) => {
    const field = within(canvasElement).getByRole("textbox", { name: "Page title" });
    await userEvent.type(field, "Weekly plan");
    await expect(args.onChange).toHaveBeenCalled();
    await expect(field).toHaveValue("Weekly plan");
  },
};
