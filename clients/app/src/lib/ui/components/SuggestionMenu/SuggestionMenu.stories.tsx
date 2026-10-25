import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { SuggestionMenu } from "./SuggestionMenu";

const meta = {
  title: "Components/SuggestionMenu",
  component: SuggestionMenu,
  args: {
    label: "Insert a block",
    anchor: { x: 24, y: 24 },
    empty: "Nothing matches.",
    active: "heading1",
    onPick: fn(),
    groups: [
      {
        id: "basic",
        label: "Basic",
        items: [
          { id: "text", label: "Text", glyph: "T" },
          { id: "heading1", label: "Heading 1", glyph: "H1", hint: "#" },
          { id: "todo", label: "To-do", icon: "check", hint: "[]" },
          { id: "bullets", label: "Bulleted list", icon: "bullets", hint: "-" },
        ],
      },
      {
        id: "campbooks",
        label: "From Campbooks",
        items: [
          { id: "person", label: "Mention a person", glyph: "@", hint: "@" },
          { id: "attach", label: "Attach a file", icon: "attachment" },
        ],
      },
    ],
  },
} satisfies Meta<typeof SuggestionMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("option", { name: /Heading 1/u })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await userEvent.click(canvas.getByRole("option", { name: /To-do/u }));
    await expect(args.onPick).toHaveBeenCalledWith("todo");
  },
};

/** People to mention: a name and the address under it. */
export const People: Story = {
  args: {
    label: "Mention",
    active: "jonas",
    groups: [
      {
        id: "people",
        label: "People",
        items: [
          { id: "jonas", label: "Jonas Weber", detail: "jonas@example.com", icon: "people" },
          { id: "ana", label: "Ana Ribeiro", detail: "ana@example.com", icon: "people" },
        ],
      },
      {
        id: "pages",
        label: "Pages",
        items: [{ id: "acme", label: "Acme Studio", icon: "document" }],
      },
    ],
  },
};

export const Empty: Story = { args: { groups: [] } };
export const Loading: Story = { args: { groups: [], loading: "Searching…" } };
