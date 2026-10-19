import type { Meta, StoryObj } from "@storybook/react-vite";

import {
  Attachment,
  FoldedMessage,
  MessageCard,
  MetaLine,
  MoreMessages,
  QuotedText,
} from "./Message";

const meta = {
  title: "Components/Message",
  component: MessageCard,
  args: {
    from: "Sofia Pessoa",
    time: "Fri 17:42",
    children:
      "Attached the revised scope. Thursday works for a call if that is easier; otherwise comments in the doc are great.",
  },
  decorators: [
    (Story) => (
      <div className="max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof MessageCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Long file names are cut with an ellipsis. */
export const WithAttachment: Story = {
  args: { attachment: "Brightloop-Q4-scope-v3-final-with-appendix-and-pricing-options.pdf" },
};

export const FromService: Story = {
  args: {
    from: "Cloudhost",
    avatar: { kind: "service" },
    time: "12 Oct",
    children: "Your invoice CH-20418 for €148.00 is due on 16 October.",
    attachment: "CH-20418.pdf",
  },
};

export const Meta_: Story = {
  name: "Meta line",
  render: () => <MetaLine parts={["Sofia Pessoa", "Brightloop", "2 messages"]} />,
};

export const AttachmentOnly: Story = { render: () => <Attachment name="CH-20418.pdf" /> };

/** Recipients, several files and a per-message action. */
export const Full: Story = {
  args: {
    recipients: "To Sam Rivera, cc Leo Brandt",
    attachments: ["q4-plan-v3.pdf", "budget.xlsx"],
    actions: <span>1 remote image hidden</span>,
  },
};

/** An older message, folded to one line. */
export const Folded: Story = {
  render: () => (
    <FoldedMessage
      from="Sofia Pessoa"
      time="Tue 10:14"
      snippet="Hi Ana, here is the first draft of the Q4 proposal. Let me know what you think."
      onOpen={() => {
        // Demo only.
      }}
    />
  ),
};

/** A message you sent: "You → <recipient>", set in from the left on the sunken surface. */
export const Yours: Story = {
  args: {
    from: "You",
    addressee: "→ Marta Silva",
    side: "yours",
    avatar: { initials: "SR" },
    time: "Thu 17:40",
    recipients: "To Marta Silva",
    children: "Revised quote attached: €2,340 incl. VAT, two extra brackets included.",
    attachment: "quote-0412.pdf",
  },
};

/** Theirs and yours folded, as a thread shows them. */
export const FoldedSides: Story = {
  render: () => (
    <div className="flex flex-col">
      <FoldedMessage
        from="You"
        addressee="→ Marta Silva and 2 others"
        side="yours"
        avatar={{ initials: "SR" }}
        time="Thu"
        snippet="Revised quote attached: €2,340 incl. VAT, two extra brackets included."
        onOpen={() => {
          // Demo only.
        }}
      />
      <FoldedMessage
        from="Marta Silva"
        side="theirs"
        time="Tue"
        snippet="Could you send the revised quote for the kitchen shelving?"
        onOpen={() => {
          // Demo only.
        }}
      />
    </div>
  ),
};

/** The middle of a long thread, folded into one line. */
export const More: Story = {
  render: () => (
    <MoreMessages
      label="2 more messages"
      onOpen={() => {
        // Demo only.
      }}
    />
  ),
};

/** A plain-text reply with its quoted history folded under it. */
export const WithQuotedText: Story = {
  args: {
    from: "Marta Silva",
    time: "Fri 09:12",
    children: "Perfect, approved. When can you start?\n\nOn Thu, Sam Rivera wrote:",
    quoted: (
      <QuotedText showLabel="Show quoted text" hideLabel="Hide quoted text">
        {"> Revised quote attached: €2,340 incl. VAT.\n> Two extra brackets included."}
      </QuotedText>
    ),
  },
};
