import type { Meta, StoryObj } from "@storybook/react-vite";

import { Attachment, MessageCard, MetaLine } from "./Message";

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
