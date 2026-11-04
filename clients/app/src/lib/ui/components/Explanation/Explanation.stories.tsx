import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { Button } from "@notacamp/ui";

import { ExplanationCard, ExplanationLine, type ExplanationGroup } from "./Explanation";

const chip = (id: string, label: string) => ({ id, label, onSelect: fn() });

const groups: ExplanationGroup[] = [
  {
    id: "wants",
    label: "Who wants what",
    folded: true,
    items: [
      {
        id: "w1",
        lead: "Jonas Weber",
        text: "the kickoff on Thu 6 Nov at 10:00, and a revised quote that includes the second workshop.",
        chips: [chip("a", "Jonas · 29 Oct")],
      },
      {
        id: "w2",
        lead: "Rui Matos",
        text: "the list of who is coming from your side.",
        chips: [chip("b", "Rui · 30 Oct")],
      },
    ],
  },
  {
    id: "agreed",
    label: "Agreed",
    folded: true,
    items: [
      {
        id: "g1",
        text: "Kickoff on Thu 6 Nov, 10:00, at Acme Studio's office.",
        chips: [chip("c", "Rui · 28 Oct")],
      },
    ],
  },
  {
    id: "open",
    label: "Still open",
    folded: true,
    items: [
      {
        id: "o1",
        text: "The price of the second workshop: Jonas asked and nobody has answered yet.",
        chips: [chip("d", "Jonas · 29 Oct")],
      },
    ],
  },
  {
    id: "on-you",
    label: "On you",
    emphasis: true,
    items: [
      {
        id: "y1",
        text: "Send Jonas the revised quote before Thu 6 Nov.",
        chips: [chip("e", "Jonas · 29 Oct")],
      },
      {
        id: "y2",
        text: "Send Rui the list of who is coming.",
        due: "by Mon 3 Nov",
        chips: [chip("f", "Rui · 30 Oct")],
      },
    ],
  },
];

const meta = {
  title: "Components/Explanation",
  component: ExplanationCard,
  args: { title: "Scout's explanation", hideLabel: "Hide the explanation", onHide: fn() },
  parameters: { layout: "padded" },
} satisfies Meta<typeof ExplanationCard>;

export default meta;
type Story = StoryObj<typeof meta>;

const ready = {
  meta: "From 9 messages · written today 10:42",
  inShort: {
    label: "In short",
    text: "Acme Studio wants the kickoff moved to Thursday 6 November and a revised quote before then. Rui agreed to the new date; the quote is still with you.",
  },
  groups,
  more: { label: "Who wants what, agreed, open", lessLabel: "Show less" },
  actions: (
    <>
      <Button>Ask a follow-up</Button>
      <Button variant="ghost">Reply</Button>
    </>
  ),
  privacy: "Only you see this. Scout read the messages' own words, not the quoted history.",
};

/** Written: the summary, then four groups, each point with its messages. */
export const Ready: Story = { args: ready };

/** Scout is reading: a skeleton, and Stop only hides the card. */
export const Writing: Story = {
  args: {
    writing: {
      label: "Scout is reading the 9 messages…",
      stopLabel: "Stop",
      onStop: fn(),
      inShortLabel: "In short",
    },
  },
};

/** The thread moved on since: the notice names who wrote, and Explain again is offered. */
export const Stale: Story = {
  args: {
    ...ready,
    notice: "Rui Matos wrote since, at 11:05. This explanation is from before.",
    actions: (
      <>
        <Button>Explain again</Button>
        <Button variant="ghost">Ask a follow-up</Button>
      </>
    ),
  },
};

/**
 * Points that come from several messages, one of them from a sender known
 * only by a long address: the chips wrap under the text, which keeps its width.
 */
export const ManySources: Story = {
  // As wide as the card is in the reader beside the inbox list.
  decorators: [
    (Story) => (
      <div style={{ maxWidth: "35rem" }}>
        <Story />
      </div>
    ),
  ],
  args: {
    ...ready,
    inShort: {
      label: "In short",
      text: "Hugo outlined the closing steps and costs, and Léa has taken over the file and asked for documents and invoices.",
    },
    groups: [
      {
        id: "wants",
        label: "Who wants what",
        folded: true,
        items: [
          {
            id: "w1",
            lead: "Léa Martin",
            text: "Wants the sale invoice for the company's investments",
            chips: [chip("a", "Léa · 29 Oct")],
          },
          {
            id: "w2",
            lead: "Hugo Lambert",
            text: "Wants confirmation of the closing steps and the notary's fee before filing",
            chips: [
              chip("b", "Hugo · 23 Sep"),
              chip("c", "You · 23 Oct"),
              chip("d", "accounting-department-closures@ledger-partners.example · 30 Oct"),
            ],
          },
        ],
      },
      {
        id: "open",
        label: "Still open",
        folded: true,
        items: [
          {
            id: "o1",
            text: "Payment details for the 2025 corporate tax await the tax notice",
            chips: [chip("e", "Léa · 29 Oct")],
          },
          {
            id: "o2",
            text: "Whether to involve the parent company’s accountant in the closing",
            chips: [chip("f", "You · 21 Sep"), chip("g", "Hugo · 23 Sep")],
          },
        ],
      },
    ],
  },
};

/** The same points on a wide card: two columns, the chips still under each point. */
export const ManySourcesWide: Story = { args: { ...ManySources.args } };

/** A long thread says what it rests on. */
export const LongThread: Story = {
  args: {
    ...ready,
    meta: "From 34 messages · written today 10:42",
    notice: "From 34 messages: the newest 20 in full, the 14 before them from Scout's notes.",
  },
};

/** The daily limit was reached: the earlier explanation stays, with what happened. */
export const LimitReached: Story = {
  args: {
    ...ready,
    notice: "This workspace has used today's AI. Scout can explain this thread again tomorrow.",
  },
};

/** Nothing usable: it says so and offers to try again. */
export const Failed: Story = {
  args: {
    title: "Scout's explanation",
    notice: "Scout couldn't explain this thread.",
    actions: <Button>Try again</Button>,
  },
};

/** Coming back to a thread you asked about: one line, and nothing opens until you choose. */
export const Collapsed: StoryObj<typeof ExplanationLine> = {
  render: () => (
    <ExplanationLine
      actions={
        <>
          <Button variant="ghost">Show</Button>
          <Button>Explain again</Button>
        </>
      }
    >
      Your explanation from 10:42 · Rui Matos wrote since, at 11:05
    </ExplanationLine>
  ),
};
