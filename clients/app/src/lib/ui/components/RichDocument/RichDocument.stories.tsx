import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { DocCard, DocImage, DocToggle, MentionChip, RichDocument } from "./RichDocument";

const meta = {
  title: "Components/RichDocument",
  component: RichDocument,
  args: { children: null },
  decorators: [
    (Story) => (
      <div className="w-full max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof RichDocument>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Every block of the schema, as the editor draws it. */
export const AllBlocks: Story = {
  render: (args) => (
    <RichDocument {...args}>
      <div className="tiptap">
        <h1>Client onboarding</h1>
        <h2>Before the kickoff</h2>
        <ul data-type="taskList">
          <li data-checked="true" data-type="taskItem">
            <label>
              <input type="checkbox" defaultChecked aria-label="Done" />
            </label>
            <div>
              <p>
                Send the welcome pack to <MentionChip kind="person" label="Jonas Weber" />
              </p>
            </div>
          </li>
          <li data-checked="false" data-type="taskItem">
            <label>
              <input type="checkbox" aria-label="Not done" />
            </label>
            <div>
              <p>
                Kickoff call <MentionChip kind="event" label="Thu 30 Oct, 10:00" /> with{" "}
                <MentionChip kind="page" label="Acme Studio" href="/pages/acme" />
              </p>
            </div>
          </li>
        </ul>
        <p>
          Every new client gets the same first week: <strong>welcome pack</strong>,{" "}
          <em>contract</em>, <code>kickoff</code>, first invoice.
        </p>
        <div data-type="callout" data-tone="info">
          <p>When the signed contract arrives, Scout offers to tick that box.</p>
        </div>
        <blockquote>
          <p>Keep this page as the checklist.</p>
        </blockquote>
        <ul>
          <li>
            <p>Brand files</p>
          </li>
          <li>
            <p>Copy sign-off</p>
          </li>
        </ul>
        <ol>
          <li>
            <p>Welcome pack</p>
          </li>
          <li>
            <p>Contract</p>
          </li>
        </ol>
        <hr />
        <pre>
          <code>{"const kickoff = new Date(2026, 9, 30, 10, 0);"}</code>
        </pre>
        <DocCard icon="attachment" title="brand-guide.pdf" meta="2.4 MB" href="/f" download="x" />
        <DocCard icon="mail" title="Kickoff agenda" meta="Thread · 4 messages" href="/t" />
        <DocCard icon="lock" title="Something you can't open" state="closed" />
      </div>
    </RichDocument>
  ),
};

/** The three states of a mention: ready, still loading, and closed to this viewer. */
export const Mentions: Story = {
  render: (args) => (
    <RichDocument {...args}>
      <p>
        <MentionChip kind="thread" label="Kickoff agenda" href="/t" onOpen={fn()} />{" "}
        <MentionChip kind="task" label="…" state="loading" />{" "}
        <MentionChip kind="thread" label="A thread you can't open" state="closed" />
      </p>
    </RichDocument>
  ),
};

/** A wide code block scrolls inside its box. */
export const WideCode: Story = {
  render: (args) => (
    <RichDocument {...args}>
      <pre tabIndex={0}>
        <code>
          {
            "const a = 'a very long line that goes well past the edge of a phone screen and keeps going and going';"
          }
        </code>
      </pre>
    </RichDocument>
  ),
};

export const Image: Story = {
  render: (args) => (
    <RichDocument {...args}>
      <DocImage
        src="data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw=="
        alt="A pixel"
      />
      <DocImage src="" alt="Something you can't open" state="closed" />
    </RichDocument>
  ),
};

function ToggleExample() {
  const [open, setOpen] = useState(false);
  return (
    <DocToggle
      open={open}
      label={open ? "Hide what is inside" : "Show what is inside"}
      onToggle={() => {
        setOpen((value) => !value);
      }}
    >
      <div data-node-view-content="">
        <p>Holiday rules</p>
        <p>Twenty-two days, booked a month ahead.</p>
      </div>
    </DocToggle>
  );
}

/** A toggle hides what follows its first line. */
export const Toggle: Story = {
  render: (args) => (
    <RichDocument {...args}>
      <ToggleExample />
    </RichDocument>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const arrow = canvas.getByRole("button", { name: "Show what is inside" });
    await expect(arrow).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(arrow);
    await expect(canvas.getByRole("button", { name: "Hide what is inside" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  },
};
