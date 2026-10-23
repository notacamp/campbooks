import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { useShortcut } from "~/lib/shortcuts";

import { Button } from "../Button/Button";

import { CommandMenu, type Command, type MenuQuery, type MenuSection } from "./CommandMenu";

const meta = {
  title: "Components/CommandMenu",
  component: CommandMenu,
  parameters: { layout: "fullscreen" },
  args: { open: true, onClose: fn() },
} satisfies Meta<typeof CommandMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

const suggestions: Command[] = [
  {
    id: "reply-sofia",
    label: "Reply to Sofia Pessoa",
    group: "Suggested",
    shortcut: "item.reply",
    onSelect: fn(),
  },
  {
    id: "hold-q4",
    label: "Hold time for the Q4 review",
    group: "Suggested",
    shortcut: "item.hold",
    onSelect: fn(),
  },
];

/** What a screen binds shows up in the menu with its keys (here: the G-then-X places). */
function Screen({
  onAsk,
  useSection,
}: {
  onAsk?: (query: string) => void;
  useSection?: (menu: MenuQuery) => MenuSection | undefined;
}) {
  const [open, setOpen] = useState(true);
  const go = fn();
  useShortcut("nav.home", go);
  useShortcut("nav.inbox", go);
  useShortcut("nav.schedule", go);
  useShortcut("item.done", go);
  useShortcut("command.open", () => {
    setOpen(true);
  });
  return (
    <div className="p-4">
      <Button
        shortcut="command.open"
        onClick={() => {
          setOpen(true);
        }}
      >
        Search or ask
      </Button>
      <CommandMenu
        open={open}
        onClose={() => {
          setOpen(false);
        }}
        commands={suggestions}
        onAsk={onAsk}
        useSection={useSection}
      />
    </div>
  );
}

export const Open: Story = { render: () => <Screen /> };

/** Typing filters (accents ignored); ↓ and ↵ run a command. */
export const Filtering: Story = {
  render: () => <Screen />,
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    const input = await body.findByRole("combobox");
    await userEvent.type(input, "inb");
    await expect(body.getAllByRole("option")).toHaveLength(1);
    await expect(body.getByRole("option", { name: /Inbox/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(body.queryByRole("dialog")).not.toBeInTheDocument());
  },
};

/** With a query, the last row hands it to the assistant. */
export const Ask: Story = {
  render: () => <Screen onAsk={fn()} />,
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    await userEvent.type(await body.findByRole("combobox"), "Anything I still owe Brightloop?");
    await expect(
      body.getByRole("option", { name: /Ask Scout: Anything I still owe Brightloop\?/ }),
    ).toBeVisible();
  },
};

/** A screen's own section: matching rows first, then the commands, then Ask, under one cursor. */
const useConversations = ({ query }: MenuQuery): MenuSection | undefined =>
  query.trim() === ""
    ? undefined
    : {
        group: "Conversations",
        chips: [{ id: "from:ana", label: "From ana", onRemove: fn() }],
        onSubmitAll: fn(),
        options: [
          { id: "lease", label: "Lease renewal for the Rua Direita flat", onSelect: fn() },
          { id: "all", label: `All results for “${query.trim()}”`, onSelect: fn() },
        ],
      };

export const WithSection: Story = {
  render: () => <Screen onAsk={fn()} useSection={useConversations} />,
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    const input = await body.findByRole("combobox");
    await userEvent.type(input, "lease");
    const options = body.getAllByRole("option");
    await expect(options[0]).toHaveTextContent("Lease renewal");
    await expect(options.at(-1)).toHaveTextContent("Ask Scout: lease");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    await expect(input).toHaveAttribute("aria-activedescendant", options[2]?.id);
    await expect(body.getByRole("button", { name: "Remove filter: From ana" })).toBeVisible();
  },
};

/** Escape closes it and focus returns to what opened it. */
export const EscapeCloses: Story = {
  render: () => <Screen />,
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    await body.findByRole("dialog", { name: "Command menu" });
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(body.queryByRole("dialog")).not.toBeInTheDocument());
  },
};
