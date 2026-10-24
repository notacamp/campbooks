import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";
import { HowDecided } from "../HowDecided/HowDecided";
import { Pill } from "../Pill/Pill";
import { Tag } from "../Tag/Tag";

import { Note } from "./Note";

const how = (
  <HowDecided
    rows={[
      {
        question: "Needs a reply",
        answer: "Yes. It asks you to reply (“could you confirm”).",
        source: "Rules · 0.90 · acts from 0.88",
      },
      {
        question: "Mentions a date",
        answer: "Yes (“Wednesday”).",
        source: "Rules · 0.96 · acts from 0.75",
      },
    ]}
  />
);

const meta = {
  title: "Components/Note",
  component: Note,
  args: {
    children: "Marta Ferreira is waiting for your reply.",
    quote: {
      text: "Could you confirm Thursday’s walkthrough? The builders need to know by Wednesday.",
      attribution: "Marta Ferreira, Mon 12 Oct",
    },
    chips: (
      <>
        <Pill signal="reply">Needs a reply</Pill>
        <Tag>Due tomorrow</Tag>
      </>
    ),
    actions: (
      <>
        <Button variant="secondary" keys="E">
          Done
        </Button>
        <Button variant="ghost" keys="X">
          Not needed
        </Button>
      </>
    ),
    how,
    links: [{ icon: "document", title: "Walkthrough-plan.pdf", kind: "Document", onSelect: fn() }],
  },
} satisfies Meta<typeof Note>;

export default meta;
type Story = StoryObj<typeof meta>;

/** What a conversation needs: the sentence, the sender's own words, chips and actions. */
export const Default: Story = {};

/** "How Campbooks decided" is the only place the numbers show. */
export const HowOpen: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByText("How Campbooks decided"));
    await expect(canvas.getByText("Rules · 0.90 · acts from 0.88")).toBeVisible();
  },
};

/** A warning: the reasons first, the sender by address, then the quote. */
export const Warning: Story = {
  args: {
    visor: "alert",
    children: "This may be phishing. Don’t use its links.",
    reasons: [
      "The sender’s domain imitates yours (at3lier-sol.example).",
      "A link goes somewhere other than it says (verify-mailbox.example).",
    ],
    quote: {
      text: "Your mailbox will be closed today unless you verify your account using the link below.",
      attribution: "it-support@at3lier-sol.example, today 07:52",
    },
    chips: <Pill signal="review">Looks suspicious</Pill>,
    links: [],
  },
};

/** Unsure: the assistant asks instead of acting. */
export const Question: Story = {
  args: {
    visor: "unsure",
    children: "Does Inês Rocha need a reply?",
    choice: {
      prompt: "Campbooks is not sure.",
      options: [
        { label: "Yes", keys: "1", onSelect: fn() },
        { label: "No", keys: "2", onSelect: fn() },
      ],
    },
    actions: undefined,
    chips: undefined,
    links: [],
  },
};

/** A stream suggestion, with Move in the footer. */
export const Suggestion: Story = {
  args: {
    visor: "unsure",
    children: "Campbooks thinks this belongs in Newsletters. Nothing moves until you say so.",
    quote: undefined,
    chips: undefined,
    actions: undefined,
    links: [],
    footer: <Button variant="link">Move</Button>,
  },
};

/** Phones: smaller text and visor. */
export const Compact: Story = { args: { compact: true } };

/**
 * A titled note (Scout noticed): the title and "1 of 5" on a line of their
 * own, the longer sentence in the body font, and a link under it.
 */
export const Titled: Story = {
  args: {
    visor: "happy",
    label: "Scout noticed",
    title: "Scout noticed",
    meta: "1 of 5",
    children:
      "You got 34 emails from CI (builds@ci.example.com) in the last 60 days and opened 2. They come most days. Keep them off Home and file them under Updates?",
    after: (
      <Button variant="link" icon="arrow-right">
        See the 34 emails
      </Button>
    ),
    quote: undefined,
    chips: undefined,
    how: undefined,
    links: [],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Scout noticed", { selector: "span" })).toBeVisible();
    await expect(canvas.getByText("1 of 5")).toBeVisible();
    await expect(canvas.getByRole("button", { name: "See the 34 emails" })).toBeVisible();
  },
};
