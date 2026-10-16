import type { Meta, StoryObj } from "@storybook/react-vite";

import { RulePreview } from "./RulePreview";

const meta = {
  title: "Components/RulePreview",
  component: RulePreview,
  parameters: { layout: "padded" },
  decorators: [
    (Story) => (
      <div className="max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof RulePreview>;

export default meta;
type Story = StoryObj<typeof meta>;

export const WithMatches: Story = {
  args: {
    tag: "Rule",
    sentence: "Mail from Rita Gomes (rita@silva-contas.example) always stays in your inbox.",
    summary: "Matches 41 emails in ines@ateliersousa.example",
    samplesLabel: "Emails it matches",
    samples: [
      { subject: "Q3 VAT figures", sender: "Rita Gomes", when: "12 Oct" },
      { subject: "September invoices", sender: "Rita Gomes", when: "2 Oct" },
      {
        subject: "Year-end close: what we still need from you",
        sender: "Rita Gomes",
        when: "18 Sep",
      },
    ],
  },
};

export const Counting: Story = {
  args: {
    tag: "Rule",
    sentence: "Newsletters and mailing lists never need you.",
    pending: true,
  },
};

export const NoMailYet: Story = {
  args: {
    tag: "Rule",
    sentence: "Mail from anyone at pessoa-obras.example always stays in your inbox.",
    summary: "No mail yet. It applies as mail arrives.",
  },
};
