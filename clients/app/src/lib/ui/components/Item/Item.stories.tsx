import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";
import { ItemActions } from "../ItemActions/ItemActions";

import { Item, ItemDetail, SwipeAction } from "./Item";

const meta = {
  title: "Components/Item",
  component: Item,
  args: {
    who: "Sofia Pessoa · Brightloop",
    what: "Waiting on your review of the Q4 proposal; offers a call on Thursday",
    avatar: { name: "Sofia Pessoa" },
    due: "Thu",
    tags: ["2 messages", "Brightloop-Q4-scope-v3.pdf · 2.4 MB", "€18,500 proposal"],
    onToggle: fn(),
  },
  decorators: [
    (Story) => (
      <div className="flex max-w-measure flex-col">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Item>;

export default meta;
type Story = StoryObj<typeof meta>;

const detail = (
  <ItemDetail
    quoteFrom="Sofia, Friday 17:42"
    quote="Attached the revised scope. We moved the launch to 30 November and kept the price at €18,500. Thursday works for a call if that is easier."
    context="Version 3 moves the timeline two weeks; the price is unchanged. You held Thursday 09:00 for it."
    actions={
      <>
        <Button variant="primary" shortcut="list.open">
          Open proposal
        </Button>
        <Button shortcut="item.reply">Reply</Button>
        <Button variant="ghost" shortcut="item.snooze">
          Snooze…
        </Button>
        <Button variant="ghost" shortcut="item.done">
          Done
        </Button>
      </>
    }
  />
);

export const Default: Story = {};

const actions = (
  <ItemActions
    label="Actions for Sofia Pessoa"
    own={[{ id: "reply", label: "Reply", key: "item.choice1" }]}
    universal={[
      { id: "done", label: "Done", key: "item.done" },
      { id: "snooze", label: "Snooze…", key: "item.snooze" },
      { id: "not_needed", label: "Not needed", key: "conversations.resolve_need" },
    ]}
    onAction={fn()}
  />
);

/** The keyboard cursor (J/K) is here: its actions show right under it. */
export const Selected: Story = {
  args: { selected: true, actions },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("group", { name: "Actions for Sofia Pessoa" })).toBeVisible();
  },
};

/** Acted on: a one-line confirmation that folds away. */
export const Confirmation: Story = {
  args: { confirmation: "Done · Sofia Pessoa", onConfirmEnd: fn() },
};

export const Expanded: Story = {
  args: { selected: true, expanded: true, actions, detail },
  play: async ({ canvasElement }) => {
    const row = within(canvasElement).getByRole("button", { name: /Sofia Pessoa · Brightloop/ });
    await expect(row).toHaveAttribute("aria-expanded", "true");
  },
};

export const Collapsible: Story = {
  args: { detail },
  play: async ({ canvasElement, args }) => {
    await userEvent.click(within(canvasElement).getByRole("button", { name: /Sofia Pessoa/ }));
    await expect(args.onToggle).toHaveBeenCalledOnce();
  },
};

/** Due soon is drawn in the danger colour. */
export const DueSoon: Story = {
  args: {
    who: "Bankly",
    avatar: { name: "Bankly", kind: "service" },
    due: "Today",
    soon: true,
    what: "New sign-in from Lisbon, Sunday 21:14, on a Windows laptop",
    tags: ["Security alert", "Not a device Campbooks has seen"],
  },
};

/** Phone: a link to the conversation, with the swipe actions revealed. */
export const Swiped: Story = {
  args: {
    who: "Sofia Pessoa",
    what: "Waiting on your review of the Q4 proposal",
    tags: [],
    swipeActions: (
      <>
        <SwipeAction tone="later" icon="clock" label="Snooze" onClick={fn()} />
        <SwipeAction tone="done" icon="check" label="Done" onClick={fn()} />
      </>
    ),
  },
};

export const AsLink: Story = {
  args: {
    href: "#conversation",
    tags: [],
    who: "Rui Matos",
    what: "Send two August receipts",
  },
};

/** What needs you: the sentence, then the sender's own words on one line. */
export const WithExcerpt: Story = {
  args: {
    who: "Marta Ferreira · Casa Lume",
    what: "Marta Ferreira is waiting for your reply",
    excerpt: "Could you confirm Thursday’s walkthrough? The builders need to know by Wednesday.",
    avatar: { name: "Marta Ferreira" },
    due: "Due tomorrow",
    soon: true,
    tags: [],
  },
};

/** Opened with Scout's summary above the email (here a stand-in for it). */
export const SummaryOpen: Story = {
  args: {
    who: "Marta Ferreira",
    what: "Marta asks you to confirm Thursday’s walkthrough.",
    avatar: { name: "Marta Ferreira" },
    due: "Due Wed",
    tags: [],
    expanded: true,
    detail: (
      <ItemDetail
        summaryLabel="Scout’s summary"
        summary="Marta is arranging the walkthrough with the builders. She proposes Thursday at 10:00 and needs your answer by Wednesday."
        mail={
          <p>
            Hi Sam, could you confirm Thursday’s walkthrough? The builders need to know by
            Wednesday.
          </p>
        }
        actions={<Button variant="secondary">Open conversation</Button>}
      />
    ),
  },
};

/** A warning, opened: its tells before the quote. */
export const WarningOpen: Story = {
  args: {
    who: "it-support@at3lier-sol.example",
    what: "This may be phishing",
    avatar: { name: "IT Support", kind: "service" },
    due: undefined,
    tags: [],
    expanded: true,
    detail: (
      <ItemDetail
        reasons={[
          "The sender’s domain imitates yours (at3lier-sol.example).",
          "A link goes somewhere other than it says (verify-mailbox.example).",
        ]}
        quoteFrom="it-support@at3lier-sol.example, today 07:52"
        quote="Your mailbox will be closed today unless you verify your account using the link below."
        actions={<Button variant="secondary">It’s safe</Button>}
      />
    ),
  },
};
