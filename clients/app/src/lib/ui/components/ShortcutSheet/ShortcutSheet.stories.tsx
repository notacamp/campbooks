import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { useShortcut } from "~/lib/shortcuts";

import { Button } from "../Button/Button";

import { ShortcutSheet } from "./ShortcutSheet";

const meta = {
  title: "Components/ShortcutSheet",
  component: ShortcutSheet,
  parameters: { layout: "fullscreen" },
  args: { open: true, onClose: fn() },
} satisfies Meta<typeof ShortcutSheet>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Every shortcut the registry knows, grouped, keys read from the registry. */
export const Open: Story = {};

function Screen() {
  const [open, setOpen] = useState(false);
  useShortcut("sheet.open", () => {
    setOpen((current) => !current);
  });
  return (
    <div className="p-4">
      <Button
        variant="ghost"
        icon="keyboard"
        shortcut="sheet.open"
        onClick={() => {
          setOpen(true);
        }}
      >
        Keyboard shortcuts
      </Button>
      <ShortcutSheet
        open={open}
        onClose={() => {
          setOpen(false);
        }}
      />
    </div>
  );
}

/** ? opens the sheet; Esc closes it. */
export const QuestionMarkOpens: Story = {
  render: () => <Screen />,
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    await userEvent.keyboard("?");
    await expect(await body.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeVisible();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(body.queryByRole("dialog")).not.toBeInTheDocument());
  },
};
