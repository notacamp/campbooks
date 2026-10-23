import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { DatePicker, type DatePickerProps } from "./DatePicker";

const NOW = new Date(2026, 9, 13, 10);

/** The picker with its own state, so picking shows. */
function Demo(props: DatePickerProps) {
  const [value, setValue] = useState(props.value);
  return (
    <div className="flex w-sidebar flex-col">
      <DatePicker
        {...props}
        value={value}
        onChange={(day) => {
          setValue(day);
          props.onChange(day);
        }}
      />
    </div>
  );
}

const meta = {
  title: "Components/DatePicker",
  component: DatePicker,
  args: { label: "Due", value: "2026-10-26", onChange: fn(), now: NOW },
  render: (args) => <Demo {...args} />,
} satisfies Meta<typeof DatePicker>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A chosen day, in words. */
export const Default: Story = {};

/** No day yet: the placeholder. */
export const Empty: Story = { args: { value: undefined, placeholder: "No date" } };

/** Open on the chosen day: today ringed, the day filled, the keys move it. */
export const Open: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: /^Due/u }));
    await expect(canvas.getByRole("grid", { name: "October 2026" })).toBeVisible();
    await userEvent.keyboard("{ArrowRight}");
    await expect(canvas.getByRole("button", { name: /October 27, 2026/u })).toHaveFocus();
  },
};

/** Bounds: nothing before today, nothing past a month from now, and Clear. */
export const Bounded: Story = {
  args: {
    label: "Expires on",
    value: undefined,
    min: "2026-10-13",
    max: "2026-11-13",
    clearable: true,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: /^Expires on/u }));
    await expect(canvas.getByRole("button", { name: "Previous month" })).toBeDisabled();
    await expect(canvas.getByRole("button", { name: "Clear" })).toBeVisible();
  },
};

/** With an error under it. */
export const WithError: Story = {
  args: { label: "Expires on", value: "2026-10-01", error: "Pick a day in the future." },
};

/** The grid in place, with no field (the due chips' Pick a date). */
export const Inline: Story = {
  args: { label: "Pick a date", value: undefined, inline: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: /October 20, 2026/u }));
    await expect(
      canvas.getByRole("button", { name: /October 20, 2026/u }).closest("[role='gridcell']"),
    ).toHaveAttribute("aria-selected", "true");
  },
};
