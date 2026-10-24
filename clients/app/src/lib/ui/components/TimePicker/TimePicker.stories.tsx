import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { Dialog } from "../Dialog/Dialog";
import { TextField } from "../Field/Field";

import { TimePicker, type TimePickerProps } from "./TimePicker";

/** The picker with its own state, so picking shows. */
function Demo(props: TimePickerProps) {
  const [value, setValue] = useState(props.value);
  return (
    <div className="flex w-sidebar flex-col">
      <TimePicker
        {...props}
        value={value}
        onChange={(time) => {
          setValue(time);
          props.onChange(time);
        }}
      />
    </div>
  );
}

const meta = {
  title: "Components/TimePicker",
  component: TimePicker,
  args: { label: "Starts", value: "09:30", onChange: fn() },
  render: (args) => <Demo {...args} />,
} satisfies Meta<typeof TimePicker>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A time, in the member's language. */
export const Default: Story = {};

/** No time yet. */
export const Empty: Story = { args: { value: undefined } };

/** Open: every quarter of an hour, the field's time active; a typed time jumps there. */
export const Open: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const field = canvas.getByRole("combobox", { name: "Starts" });
    await userEvent.click(field);
    await expect(canvas.getByRole("listbox", { name: "Starts" })).toBeVisible();
    await userEvent.clear(field);
    await userEvent.type(field, "2115{Enter}");
    await expect((field as HTMLInputElement).value).toMatch(/^9:15\sPM$/u);
  },
};

/** An end time: nothing before its start, every half hour. */
export const AfterStart: Story = {
  args: { label: "Ends", value: "10:30", min: "10:00", step: 30 },
};

/** With an error under it. */
export const WithError: Story = {
  args: { label: "Ends", value: "08:00", error: "It ends before it starts." },
};

/**
 * Near the bottom of a dialog: the list opens over the dialog (in the top
 * layer), whole, and typing replaces the time shown ("930", Enter).
 */
export const InADialog: Story = {
  args: { label: "Starts", value: "09:00" },
  render: (args) => (
    <Dialog open onClose={fn()} title="New event" visor={null}>
      <div className="flex flex-col gap-3">
        <TextField label="Title" defaultValue="Walkthrough at Rua das Flores" />
        <TextField label="Place" defaultValue="Rua das Flores 12, Lisboa" />
        <TextField label="Notes" defaultValue="Bring the signed lease" />
        <Demo {...args} />
      </div>
    </Dialog>
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const field = page.getByRole("combobox", { name: "Starts" });
    await userEvent.click(field);
    await expect(page.getByRole("listbox", { name: "Starts" })).toBeVisible();
    await userEvent.keyboard("930{Enter}");
    await expect((field as HTMLInputElement).value).toMatch(/^9:30\sAM$/u);
    await userEvent.keyboard("{Escape}");
    await expect(page.getByRole("dialog", { name: "New event" })).toBeVisible();
  },
};
