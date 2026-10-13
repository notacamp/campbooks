import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { Segmented } from "./Segmented";

const noop = () => {
  // Demo only: nothing to do.
};

const meta = {
  title: "Components/Segmented",
  component: Segmented,
} satisfies Meta<typeof Segmented>;

export default meta;
type Story = StoryObj<typeof meta>;

function Demo() {
  const [value, setValue] = useState<"all" | "unread">("all");
  return (
    <Segmented
      label="Show"
      value={value}
      onChange={setValue}
      options={[
        { value: "all", label: "All" },
        { value: "unread", label: "Unread" },
      ]}
    />
  );
}

export const Default: Story = {
  args: { label: "Show", value: "all", onChange: noop, options: [] },
  render: () => <Demo />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Unread" }));
    await expect(canvas.getByRole("button", { name: "Unread" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await expect(canvas.getByRole("button", { name: "All" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  },
};
