import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { SearchField, type SearchFieldChip } from "./SearchField";

const meta = {
  title: "Components/SearchField",
  component: SearchField,
  args: {
    label: "Search conversations",
    placeholder: "Search conversations",
    value: "",
    onValueChange: fn(),
    keys: "/",
  },
  decorators: [
    (Story) => (
      <div className="max-w-overlay">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof SearchField>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {};

const chips: SearchFieldChip[] = [
  { id: "from", label: "From ana", onRemove: fn() },
  { id: "attachment", label: "Has attachment", onRemove: fn() },
  { id: "unread", label: "Unread", onRemove: fn() },
];

/** The filters it understood, removable; they wrap on a phone. */
export const WithChips: Story = {
  args: { value: "invoice from:ana has:attachment is:unread", chips, onClear: fn() },
};

export const Overlay: Story = {
  args: { variant: "overlay", keys: "escape", value: "lease", chips },
};

/** Typing, clearing and removing a chip, with a live value. */
export const Interactive: Story = {
  render: function Render(args) {
    const [value, setValue] = useState("invoice");
    const [shown, setShown] = useState(chips);
    return (
      <SearchField
        {...args}
        value={value}
        onValueChange={setValue}
        onClear={() => {
          setValue("");
        }}
        chips={shown.map((chip) => ({
          ...chip,
          onRemove: () => {
            setShown(shown.filter((other) => other.id !== chip.id));
          },
        }))}
      />
    );
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Remove filter: From ana" }));
    await expect(canvas.queryByText("From ana")).toBeNull();
    await userEvent.click(canvas.getByRole("button", { name: "Clear search" }));
    await expect(canvas.getByRole("searchbox")).toHaveValue("");
  },
};
