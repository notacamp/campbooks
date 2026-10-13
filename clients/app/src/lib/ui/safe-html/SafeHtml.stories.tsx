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

/**
 * The server's placeholder for a remote image it held back (a fixed `data:`
 * picture at the image's declared size), until "Show images".
 */
const PLACEHOLDER =
  "data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%3E%3Crect%20width='100%25'%20height='100%25'%20rx='6'%20fill='%23f1f0ee'/%3E%3Csvg%20x='50%25'%20y='50%25'%20overflow='visible'%3E%3Cg%20transform='translate(-10%20-10)'%20fill='none'%20stroke='%238f8d89'%20stroke-width='1.5'%20stroke-linecap='round'%20stroke-linejoin='round'%3E%3Crect%20x='2'%20y='3'%20width='16'%20height='14'%20rx='2'/%3E%3Ccircle%20cx='7'%20cy='8'%20r='1.5'/%3E%3Cpath%20d='m2.5%2015%204.5-4.5%203.5%203.5%202-2%204.5%204.5'/%3E%3C/g%3E%3C/svg%3E%3C/svg%3E";

/** A remote image the server held back: the placeholder, until "Show images". */
export const ImageBlocked: Story = {
  args: {
    html: `<img alt="The Weekly Ledger" width="480" height="100" style="width:100%;height:auto" src="${PLACEHOLDER}">${NEWSLETTER}<p>Logo: <img alt="Logo" src="${PLACEHOLDER}" width="48" height="48"></p>`,
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
