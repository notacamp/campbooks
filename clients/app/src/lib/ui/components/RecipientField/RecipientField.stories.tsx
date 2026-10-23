import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { RecipientField, type Recipient, type RecipientSuggestion } from "./RecipientField";

/** The people you exchange mail with, written to most first (fictional). */
const PEOPLE: RecipientSuggestion[] = [
  { name: "Marta Ferreira", address: "marta@ferreira-studio.example" },
  { name: "Mário Lopes", address: "mario.lopes@lopes-arquitetos.example" },
  { name: "Omar Haddad", address: "omar@northwind-studio.example" },
  { address: "marisa@harbour-books.example" },
];

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();

function Controlled({
  initial = [],
  suggesting = false,
  loading = false,
}: {
  initial?: Recipient[];
  /** Suggest {@link PEOPLE} whose name or address holds what is typed. */
  suggesting?: boolean;
  /** The suggestions are on their way, and none has arrived yet. */
  loading?: boolean;
}) {
  const [recipients, setRecipients] = useState<Recipient[]>(initial);
  const [query, setQuery] = useState("");
  const typed = fold(query.trim());
  const suggestions =
    loading || typed.length < 2
      ? []
      : PEOPLE.filter((person) => fold(`${person.address} ${person.name ?? ""}`).includes(typed));
  return (
    <RecipientField
      label="To"
      recipients={recipients}
      onChange={setRecipients}
      {...(suggesting ? { suggestions, onQueryChange: setQuery, loadingSuggestions: loading } : {})}
    />
  );
}

const meta = {
  title: "Components/RecipientField",
  component: Controlled,
  decorators: [
    (Story) => (
      <div className="max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Controlled>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {};

export const WithRecipients: Story = {
  args: {
    initial: [
      { name: "Maya Okafor", address: "maya@northwind-studio.example" },
      { address: "leo@example.com" },
    ],
  },
};

/** Enter, a comma or Tab turns what you typed into a chip; Backspace removes the last one. */
export const AddWithTheKeyboard: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("textbox", { name: "To" });
    await userEvent.type(input, "ana@example.com{Enter}Leo Park <leo@example.com>,");
    await expect(canvas.getByText("ana@example.com")).toBeVisible();
    await expect(canvas.getByText("Leo Park")).toBeVisible();
    await userEvent.type(input, "{Backspace}");
    await expect(canvas.queryByText("Leo Park")).not.toBeInTheDocument();
  },
};

export const InvalidAddress: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("textbox", { name: "To" });
    await userEvent.type(input, "not an address{Enter}");
    await expect(input).toHaveAttribute("aria-invalid", "true");
    await expect(canvas.getByText(/This is not an email address/u)).toBeVisible();
  },
};

/** Typing suggests the people you exchange mail with; ↓ and Enter add one. */
export const Suggestions: Story = {
  args: { suggesting: true, initial: [{ name: "Leo Park", address: "leo@example.com" }] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("combobox", { name: "To" });
    await userEvent.type(input, "mar");
    const list = canvas.getByRole("listbox", { name: "Suggestions for To" });
    await expect(within(list).getAllByRole("option")).toHaveLength(4);
    await expect(within(list).getAllByRole("option")[0]).toHaveAttribute("aria-selected", "true");
    await userEvent.keyboard("{ArrowDown}");
    await expect(within(list).getAllByRole("option")[1]).toHaveAttribute("aria-selected", "true");
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByText("Mário Lopes")).toBeVisible();
    await expect(canvas.queryByRole("listbox")).not.toBeInTheDocument();
    // Picked people are not suggested again.
    await userEvent.type(input, "mar");
    await expect(canvas.getAllByRole("option")).toHaveLength(3);
  },
};

/** The list open, as it first shows. */
export const SuggestionsOpen: Story = {
  args: { suggesting: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(canvas.getByRole("combobox", { name: "To" }), "ma");
    await expect(canvas.getByRole("listbox", { name: "Suggestions for To" })).toBeVisible();
  },
};

/** Esc closes the list and leaves what was typed; Enter then adds it. */
export const SuggestionsClosedWithEsc: Story = {
  args: { suggesting: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("combobox", { name: "To" });
    await userEvent.type(input, "marisa@harbour-books.example");
    await expect(canvas.getByRole("listbox")).toBeVisible();
    await userEvent.keyboard("{Escape}{Enter}");
    await expect(canvas.getByText("marisa@harbour-books.example")).toBeVisible();
    await expect(canvas.queryByRole("listbox")).not.toBeInTheDocument();
  },
};

/** Before the first suggestions arrive. */
export const SuggestionsSearching: Story = {
  args: { suggesting: true, loading: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(canvas.getByRole("combobox", { name: "To" }), "mar");
    await expect(canvas.getByRole("status")).toHaveTextContent("Searching…");
  },
};
