import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";

import { NoteField } from "./NoteField";

const meta = {
  title: "Components/NoteField",
  component: NoteField,
  args: {
    label: "About Marta, and how to write to them",
    placeholder: "e.g. My landlord. Formal, in Portuguese.",
    value: "",
    onChange: fn(),
    maxLength: 500,
    counter: (left: number) => `${String(left)} characters left`,
    onSubmit: fn(),
    onCancel: fn(),
  },
  parameters: { layout: "padded" },
} satisfies Meta<typeof NoteField>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Empty, with an example as the placeholder; it takes the focus. */
export const Empty: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("textbox", { name: "About Marta, and how to write to them" }),
    ).toHaveFocus();
  },
};

/** A note being written. */
export const Written: Story = {
  args: { value: "My landlord. Formal, in Portuguese, and she prefers a call on weekdays." },
};

/** Near the limit, a counter says how much is left. */
export const NearTheLimit: Story = {
  args: { value: "a".repeat(480) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("20 characters left")).toBeVisible();
  },
};

/** While it is kept: read-only. */
export const Saving: Story = {
  args: { value: "My landlord.", disabled: true },
};

/** It could not be kept: said under the field. */
export const Failed: Story = {
  args: { value: "My landlord.", error: "Scout couldn't keep this note. Try again." },
};

function Flow() {
  const [value, setValue] = useState("");
  const [kept, setKept] = useState<string | undefined>();
  return (
    <form
      className="flex flex-col gap-2"
      onSubmit={(event) => {
        event.preventDefault();
        setKept(value);
      }}
    >
      <NoteField
        label="About Marta, and how to write to them"
        value={value}
        onChange={setValue}
        maxLength={500}
        counter={(left) => `${String(left)} characters left`}
        onSubmit={() => {
          setKept(value);
        }}
        onCancel={() => {
          setValue("");
        }}
      />
      <div className="flex gap-2">
        <Button type="submit" variant="primary" keys="mod+enter">
          Save
        </Button>
        <Button variant="ghost" shortcut="overlay.close">
          Cancel
        </Button>
      </div>
      {kept === undefined ? null : <p>Kept: {kept}</p>}
    </form>
  );
}

/** With Save (⌘↵) and Cancel (Esc), as the reply dialog and Scout's memory show it. */
export const WithItsButtons: StoryObj = {
  render: () => <Flow />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(canvas.getByRole("textbox"), "My landlord.");
    await userEvent.keyboard("{Control>}{Enter}{/Control}");
    await expect(canvas.getByText("Kept: My landlord.")).toBeVisible();
  },
};
