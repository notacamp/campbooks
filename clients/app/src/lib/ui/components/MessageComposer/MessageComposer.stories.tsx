import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import type { Recipient } from "../RecipientField/RecipientField";

import { MessageComposer, type ComposerAttachment } from "./MessageComposer";

interface DemoProps {
  title: string;
  to?: Recipient[];
  cc?: Recipient[];
  subject?: string | undefined;
  body?: string;
  quoted?: string | undefined;
  attachments?: ComposerAttachment[];
  status?: string;
  error?: string;
  sending?: boolean;
  focus?: "to" | "body";
  onSend: () => void;
  onClose: () => void;
  onDiscard: () => void;
}

const QUOTE =
  "On Fri 10 Oct 2026, 17:42, Maya Okafor wrote:\n> Could you review the budget section by Friday?\n> Thanks, Maya";

/** The composer with its own state, as a screen would hold it. */
function Demo({
  to = [],
  cc = [],
  subject,
  body = "",
  quoted,
  attachments = [],
  ...rest
}: DemoProps) {
  const [state, setState] = useState({
    to,
    cc,
    bcc: [] as Recipient[],
    subject,
    body,
    quoted,
    attachments,
  });
  return (
    <MessageComposer
      {...rest}
      from="Sam Rivers <sam@example.com>"
      to={state.to}
      onToChange={(next) => {
        setState({ ...state, to: next });
      }}
      cc={state.cc}
      onCcChange={(next) => {
        setState({ ...state, cc: next });
      }}
      bcc={state.bcc}
      onBccChange={(next) => {
        setState({ ...state, bcc: next });
      }}
      subject={state.subject}
      onSubjectChange={(next) => {
        setState({ ...state, subject: next });
      }}
      body={state.body}
      onBodyChange={(next) => {
        setState({ ...state, body: next });
      }}
      quoted={state.quoted}
      onQuotedChange={(next) => {
        setState({ ...state, quoted: next });
      }}
      attachments={state.attachments}
      onRemoveAttachment={(id) => {
        setState({ ...state, attachments: state.attachments.filter((a) => a.id !== id) });
      }}
    />
  );
}

const meta = {
  title: "Components/MessageComposer",
  component: Demo,
  args: {
    title: "Reply to Maya Okafor",
    to: [{ name: "Maya Okafor", address: "maya@northwind-studio.example" }],
    quoted: QUOTE,
    status: "Saved",
    onSend: fn(),
    onClose: fn(),
    onDiscard: fn(),
  },
  decorators: [
    (Story) => (
      <div className="max-w-content">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Reply: Story = {};

export const ReplyAll: Story = {
  args: {
    title: "Reply all",
    cc: [
      { name: "Leo Park", address: "leo@northwind-studio.example" },
      { name: "Joana Simões", address: "joana@example.com" },
    ],
  },
};

export const Forward: Story = {
  args: {
    title: "Forward",
    to: [],
    focus: "to",
    subject: "Fwd: Q4 plan: can you review by Friday?",
    quoted:
      "---------- Forwarded message ----------\nFrom: Maya Okafor <maya@northwind-studio.example>\nDate: Fri 10 Oct 2026, 17:42\nSubject: Q4 plan: can you review by Friday?\n\nCould you review the budget section by Friday?",
    attachments: [
      { id: "a1", filename: "q4-plan-with-the-budget-section-and-appendix.pdf", size: 482_133 },
      { id: "a2", filename: "notes.txt", size: 812 },
    ],
  },
};

export const NewMessage: Story = {
  args: { title: "New message", to: [], quoted: undefined, subject: "", focus: "to", status: "" },
};

export const CannotSend: Story = {
  args: { error: "Gmail refused this message. Check the addresses and try again." },
};

export const Sending: Story = {
  args: { sending: true, status: "Sending…", body: "Thursday works for me." },
};

/** ⌘↵ (Ctrl+↵) sends, ⌘B makes the selection bold, and the quoted text unfolds. */
export const WriteAndSend: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const box = canvas.getByRole<HTMLTextAreaElement>("textbox", { name: "Message" });
    await userEvent.type(box, "Thursday works.");
    box.setSelectionRange(0, 8);
    await userEvent.click(canvas.getByRole("button", { name: "Bold" }));
    await expect(box).toHaveValue("**Thursday** works.");
    await userEvent.click(canvas.getByRole("button", { name: "Show quoted text" }));
    await expect(canvas.getByRole("textbox", { name: "Quoted text" })).toBeVisible();
    await userEvent.click(box);
    // ⌘↵ on Apple platforms, Ctrl+↵ elsewhere: exactly one of them sends.
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}{Control>}{Enter}{/Control}");
    await expect(args.onSend).toHaveBeenCalledOnce();
  },
};
