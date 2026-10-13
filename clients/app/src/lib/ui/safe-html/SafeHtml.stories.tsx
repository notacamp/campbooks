import type { Meta, StoryObj } from "@storybook/react-vite";

import { SafeHtml } from "./SafeHtml";

const NEWSLETTER = `<h1 style="font-size: 24px">Five tax dates before year end</h1>
<p>Mark these in your calendar, and read <a href="https://example.com/articles" target="_blank" rel="noopener noreferrer">what changes in 2027</a>.</p>
<ol><li>15 November: quarterly VAT return</li><li>30 November: second payment on account</li></ol>`;

const meta = {
  title: "Components/SafeHtml",
  component: SafeHtml,
  args: { html: NEWSLETTER, title: "Message from The Weekly Ledger" },
  decorators: [
    (Story) => (
      <div className="max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof SafeHtml>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Sanitised HTML in the sandbox: a white page in every theme. */
export const Default: Story = {};

/** A remote image the server held back: its alt text, until "Show images". */
export const ImageBlocked: Story = {
  args: {
    html: `<img alt="The Weekly Ledger" width="480" height="100" data-blocked="remote">${NEWSLETTER}`,
  },
};

/** Remote images allowed by the frame's own policy. */
export const ImagesShown: Story = {
  args: {
    remoteImages: true,
    html: `<img alt="Banner" width="480" height="100" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='480' height='100'%3E%3Crect width='480' height='100' fill='%232f5d50'/%3E%3C/svg%3E">${NEWSLETTER}`,
  },
};

/** A long message scrolls inside its frame. */
export const Long: Story = {
  args: { html: `<p>${"A long paragraph of an email body. ".repeat(400)}</p>` },
};
