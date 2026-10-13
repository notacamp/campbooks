import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

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
