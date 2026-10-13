import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { MessageCard } from "../Message/Message";

import { AttachmentList, ImagePreview } from "./Attachments";

const FILES = [
  { id: "a1", name: "harbour-at-dusk.jpg", size: 284_211, kind: "image" as const },
  { id: "a2", name: "Invoice CH-20418.pdf", size: 18_406, kind: "pdf" as const },
  {
    id: "a3",
    name: "Brightloop-Q4-scope-v3-final-with-appendix-and-pricing-options.docx",
    size: 1_830_102,
    kind: "file" as const,
  },
  { id: "a4", name: "statement.html", size: 920, kind: "file" as const },
];

const meta = {
  title: "Components/Attachments",
  component: AttachmentList,
  args: { attachments: FILES, onDownload: fn(), onPreview: fn() },
  decorators: [
    (Story) => (
      <div className="max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof AttachmentList>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Images and PDFs have a preview; everything else (HTML too) downloads. */
export const Default: Story = {};

/** With the shortcut that moves through them (V). */
export const WithShortcut: Story = { args: { shortcut: "inbox.next_attachment" } };

/** One being fetched. */
export const Busy: Story = { args: { busy: "a1" } };

/** A download that failed. */
export const Failed: Story = {
  args: { error: "The attachment could not be downloaded. Try again." },
};

/** Under a message. */
export const InAMessage: Story = {
  render: (args) => (
    <MessageCard
      from="Ana Lopes"
      time="Fri 17:42"
      recipients="To Sam Rivera"
      files={<AttachmentList {...args} />}
    >
      The photos from Saturday, and the invoice for the venue.
    </MessageCard>
  ),
};

const PICTURE = `data:image/svg+xml,${encodeURIComponent(
  "<svg xmlns='http://www.w3.org/2000/svg' width='640' height='420'><rect width='640' height='420' fill='#2f5d50'/><circle cx='480' cy='120' r='56' fill='#f2c14e'/><path d='M0 330 160 210 300 300 420 240 640 360V420H0Z' fill='#1d3b33'/></svg>",
)}`;

/** An attached picture, shown as an image in a dialog. */
export const Preview: Story = {
  render: () => (
    <ImagePreview src={PICTURE} name="harbour-at-dusk.jpg" onClose={fn()} onDownload={fn()} />
  ),
};

/** While the picture loads. */
export const PreviewLoading: Story = {
  render: () => (
    <ImagePreview src={undefined} name="harbour-at-dusk.jpg" onClose={fn()} onDownload={fn()} />
  ),
};
