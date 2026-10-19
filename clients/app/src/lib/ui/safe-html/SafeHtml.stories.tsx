import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";

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

/** The message's frame, once it has loaded. */
function loadedFrame(canvas: HTMLElement): { frame: HTMLIFrameElement; doc: Document } {
  const frame = canvas.querySelector("iframe");
  const doc = frame?.contentDocument ?? undefined;
  if (frame === null || doc?.URL !== "about:srcdoc") {
    throw new Error("frame not loaded");
  }
  return { frame, doc };
}

/** The frame is exactly as tall as its content: nothing cut, no padding. */
async function fitsItsContent(canvas: HTMLElement): Promise<void> {
  await waitFor(() => {
    const { frame, doc } = loadedFrame(canvas);
    const content = Math.ceil(doc.documentElement.getBoundingClientRect().height);
    if (Math.abs(frame.getBoundingClientRect().height - content) > 1) {
      throw new Error("the frame is not as tall as its content yet");
    }
  });
}

/** A long message: the frame grows to its whole height, it never scrolls inside. */
export const Long: Story = {
  args: { html: `<p>${"A long paragraph of an email body. ".repeat(400)}</p>` },
  play: async ({ canvasElement }) => {
    await fitsItsContent(canvasElement);
    await expect(loadedFrame(canvasElement).frame.getBoundingClientRect().height).toBeGreaterThan(
      1400,
    );
  },
};

/**
 * A fixed 600px layout in a narrow reader: shrunk to fit, as mobile mail
 * apps do, so the page never scrolls sideways.
 */
export const WideLayout: Story = {
  args: {
    html: `<table width="600" style="width:600px" bgcolor="#eef1f4" cellpadding="24"><tr><td><h1 style="font: bold 28px Georgia, serif; color: #14213d">Thirty per cent off waterproofs</h1><p style="font: 16px/1.5 Arial, sans-serif">Every jacket, boot and bag in the autumn range, until Sunday.</p></td></tr></table>`,
  },
  decorators: [
    (Story) => (
      <div style={{ width: "343px" }}>
        <Story />
      </div>
    ),
  ],
  play: async ({ canvasElement }) => {
    await fitsItsContent(canvasElement);
    const { frame, doc } = loadedFrame(canvasElement);
    await expect(Number(doc.body.style.zoom)).toBeLessThan(1);
    await expect(doc.documentElement.scrollWidth).toBeLessThanOrEqual(frame.clientWidth + 1);
  },
};

/**
 * The message's own `@media` rules (filtered by the server) apply inside
 * the frame, against the frame's width.
 */
export const Responsive: Story = {
  args: {
    html: `<table class="container" width="600" style="width:600px" bgcolor="#f4f5f6"><tr><td style="padding:24px">A newsletter that reflows on a phone.</td></tr></table>`,
    css: "@media only screen and (max-width: 620px) {\n.container { width: 100% !important }\n}",
  },
  decorators: [
    (Story) => (
      <div style={{ width: "343px" }}>
        <Story />
      </div>
    ),
  ],
  play: async ({ canvasElement }) => {
    await fitsItsContent(canvasElement);
    const { frame, doc } = loadedFrame(canvasElement);
    // The frame's own rules, then the message's.
    await expect(doc.adoptedStyleSheets).toHaveLength(2);
    await expect(doc.body.style.zoom).toBe("");
    await expect(doc.documentElement.scrollWidth).toBeLessThanOrEqual(frame.clientWidth);
  },
};

/**
 * A reply whose quoted history the server marked: folded behind "Show quoted
 * text", which opens and closes with no script, and the frame grows to fit.
 */
export const QuotedHistory: Story = {
  args: {
    html: `<div dir="ltr">Perfect, approved. When can you start?</div><div data-cb-quoted="1"><div class="gmail_quote"><div class="gmail_attr">On Thu, Sam Rivera wrote:</div><blockquote class="gmail_quote" style="margin: 0 0 0 0.8ex; border-left: 1px solid #cccccc; padding-left: 1ex">${"Revised quote attached: €2,340 incl. VAT, two extra brackets included. ".repeat(12)}</blockquote></div></div>`,
    showQuoted: "Show quoted text",
    hideQuoted: "Hide quoted text",
  },
  play: async ({ canvasElement }) => {
    await fitsItsContent(canvasElement);
    const { frame, doc } = loadedFrame(canvasElement);
    const details = doc.querySelector("details");
    const summary = doc.querySelector("summary");
    if (details === null || summary === null) {
      throw new Error("not folded");
    }
    await expect(details.open).toBe(false);
    const folded = frame.getBoundingClientRect().height;
    summary.click();
    await expect(details.open).toBe(true);
    await waitFor(() => {
      if (frame.getBoundingClientRect().height <= folded) {
        throw new Error("the frame has not grown yet");
      }
    });
    await fitsItsContent(canvasElement);
  },
};
