import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { Highlight } from "../Highlight/Highlight";
import { Pill } from "../Pill/Pill";
import { Tag } from "../Tag/Tag";

import { ConversationRow } from "./ConversationRow";

const meta = {
  title: "Components/ConversationRow",
  component: ConversationRow,
  args: {
    from: "Sofia Pessoa",
    time: "Fri",
    subject: "Q4 proposal: can you review it?",
    snippet: "Attached the revised scope. Thursday works for a call.",
    onSelect: fn(),
  },
  decorators: [
    (Story) => (
      <div className="flex max-w-list flex-col">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ConversationRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Unread: Story = {
  args: { unread: true },
  play: async ({ canvasElement, args }) => {
    await userEvent.click(within(canvasElement).getByRole("button"));
    await expect(args.onSelect).toHaveBeenCalledOnce();
  },
};

export const Read: Story = {};

/** The one open in the reader. */
export const Current: Story = { args: { current: true, unread: true } };

export const Service: Story = {
  args: {
    from: "Cloudhost",
    avatar: { kind: "service" },
    subject: "Invoice CH-20418, €148.00",
    snippet: "Due Friday 16 October.",
    time: "12 Oct",
  },
};

/** Long names and subjects (and Portuguese lengths) truncate cleanly. */
export const LongContent: Story = {
  args: {
    unread: true,
    from: "Maria do Carmo Sant’Ana Albuquerque e Vasconcelos",
    time: "09:41",
    subject:
      "Re: Re: Fwd: Proposta de alteração ao contrato de prestação de serviços de contabilidade 2027",
    snippet: "Segue em anexo a versão revista com as alterações acordadas na reunião",
  },
};

/** Starred, with attachments, several messages and the assistant's suggestion. */
export const WithSuggestion: Story = {
  args: {
    unread: true,
    starred: true,
    count: 3,
    attachment: true,
    from: "The Weekly Ledger",
    avatar: { kind: "service" },
    subject: "Five tax dates before year end",
    snippet: "Plus what changes for freelancers in 2027.",
    suggestion: "Suggests Newsletters",
  },
};

/** The keyboard cursor (J/K) on a row that is not open. */
export const Selected: Story = { args: { selected: true } };

/** What it needs from you: the reason and its due date. */
export const NeedsYou: Story = {
  args: {
    unread: true,
    chips: (
      <>
        <Pill signal="reply">Needs a reply</Pill>
        <Tag>Due tomorrow</Tag>
      </>
    ),
  },
};

/** A bill whose date has passed. */
export const Overdue: Story = {
  args: {
    from: "Luz Energia",
    avatar: { kind: "service" },
    subject: "Invoice 7781",
    snippet: "Please pay invoice 7781 by 12 October.",
    chips: (
      <>
        <Pill signal="pay">Pay</Pill>
        <Tag tone="danger">Overdue · was due Mon</Tag>
      </>
    ),
  },
};

/** Phishing: a warning, never a to-do. */
export const Suspicious: Story = {
  args: {
    from: "it-support@at3lier-sol.example",
    avatar: { kind: "service" },
    subject: "Action required: verify your mailbox",
    snippet: "Your mailbox will be closed today unless…",
    chips: <Pill signal="review">Looks suspicious</Pill>,
  },
};

/** A search hit: the words that matched are marked in the subject and the snippet. */
export const SearchHit: Story = {
  args: {
    unread: true,
    attachment: true,
    count: 3,
    subject: (
      <Highlight
        segments={[
          { text: "Invoice ", hit: false },
          { text: "October", hit: true },
          { text: " for the studio", hit: false },
        ]}
      />
    ),
    snippet: (
      <Highlight
        segments={[
          { text: "…the ", hit: false },
          { text: "October", hit: true },
          { text: " invoice is attached, due on the 31st.", hit: false },
        ]}
      />
    ),
  },
};

/** An option of the search overlay's listbox: not in the tab order, selected by the cursor. */
export const Option: Story = {
  args: { option: true, selected: true },
  decorators: [
    (Story) => (
      <div role="listbox" aria-label="Results">
        <Story />
      </div>
    ),
  ],
};
