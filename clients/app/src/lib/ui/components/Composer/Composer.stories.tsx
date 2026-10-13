import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { Composer } from "./Composer";

const meta = {
  title: "Components/Composer",
  component: Composer,
  args: { label: "Reply to Sofia", value: "", onChange: fn(), onSend: fn(), onDraft: fn() },
  decorators: [
    (Story) => (
      <div className="max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Composer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {};

function Controlled({ onSend }: { onSend: () => void }) {
  const [value, setValue] = useState("");
  return (
    <Composer
      label="Reply to Sofia"
      value={value}
      onChange={setValue}
      onSend={onSend}
      onDraft={fn()}
    />
  );
}

/** ⌘↵ (Ctrl+↵) sends while you type; the shortcut is live only while writing. */
export const SendWithKeyboard: Story = {
  render: (args) => <Controlled onSend={args.onSend} />,
  play: async ({ canvasElement, args }) => {
    const box = within(canvasElement).getByRole("textbox", { name: "Reply to Sofia" });
    await userEvent.type(box, "Thursday works.");
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}{Control>}{Enter}{/Control}");
    await expect(args.onSend).toHaveBeenCalledOnce();
  },
};

export const WithoutDraft: Story = {
  render: (args) => (
    <Composer label={args.label} value="" onChange={args.onChange} onSend={args.onSend} />
  ),
};
