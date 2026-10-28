import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import type { Recipient, RecipientSuggestion } from "../RecipientField/RecipientField";

import {
  MessageComposer,
  type ComposerAttachment,
  type RecipientFieldName,
  type RecipientSuggester,
} from "./MessageComposer";

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
  /** Suggest the demo people in To, Cc and Bcc as you type. */
  suggesting?: boolean;
  onSend: () => void;
  onClose: () => void;
  onDiscard: () => void;
}

const QUOTE =
  "On Fri 10 Oct 2026, 17:42, Maya Okafor wrote:\n> Could you review the budget section by Friday?\n> Thanks, Maya";

/** The people you exchange mail with, written to most first (fictional). */
const PEOPLE: RecipientSuggestion[] = [
  { name: "Marta Ferreira", address: "marta@ferreira-studio.example" },
  { name: "Mário Lopes", address: "mario.lopes@lopes-arquitetos.example" },
  { name: "Omar Haddad", address: "omar@northwind-studio.example" },
];

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();

/** Suggestions as a screen gives them: one field typed in at a time. */
function useDemoSuggester(taken: readonly Recipient[]): RecipientSuggester {
  const [typing, setTyping] = useState<{ field: RecipientFieldName; text: string }>({
    field: "to",
    text: "",
  });
  const typed = fold(typing.text.trim());
  const chosen = new Set(taken.map((recipient) => recipient.address.toLowerCase()));
  const people =
    typed.length < 2
      ? []
      : PEOPLE.filter(
          (person) =>
            !chosen.has(person.address) &&
            fold(`${person.address} ${person.name ?? ""}`).includes(typed),
        );
  return {
    suggestions: (field) => (field === typing.field ? people : []),
    loading: () => false,
    onQueryChange: (field, text) => {
      setTyping({ field, text });
    },
  };
}

/** The composer with its own state, as a screen would hold it. */
function Demo({
  to = [],
  cc = [],
  subject,
  body = "",
  quoted,
  attachments = [],
  suggesting = false,
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
  const suggest = useDemoSuggester([...state.to, ...state.cc, ...state.bcc]);
  return (
    <MessageComposer
      {...rest}
      suggest={suggesting ? suggest : undefined}
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

/** A reply with formatting: bold, a link with words, a list and a struck word. */
export const FormattedReply: Story = {
  args: {
    body: "I will **renew for a year** and read [the lease](https://example.com/lease).\n\n- parking P2-14\n- rent on the ~~5th~~ 1st",
  },
};

/** At 375px the toolbar stays one row: Strikethrough, Numbers and Quote fold into More. */
export const Narrow: Story = {
  args: { body: "Thursday works for me." },
  decorators: [
    (Story) => (
      <div style={{ width: "375px" }}>
        <Story />
      </div>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: "More formatting" })).toBeVisible();
  },
};

/** ⇧⌘E (Ctrl+Shift+E) opens the same draft as a sheet; Esc puts it back. */
export const Expanded: Story = {
  args: { body: "Thursday works for me." },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Expand" }));
    await expect(canvas.getByRole("button", { name: "Shrink" })).toBeVisible();
  },
};

export const CannotSend: Story = {
  args: { error: "Your mail provider refused this message. Check the addresses and try again." },
};

export const Sending: Story = {
  args: { sending: true, status: "Sending…", body: "Thursday works for me." },
};

/** ⌘↵ (Ctrl+↵) sends, ⌘B makes the selection bold, and the quoted text unfolds. */
export const WriteAndSend: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const box = canvas.getByRole("textbox", { name: "Message" });
    await userEvent.type(box, "Thursday works.");
    await userEvent.keyboard("{Shift>}{Home}{/Shift}");
    await userEvent.click(canvas.getByRole("button", { name: "Bold" }));
    await expect(box.querySelector("strong")).toHaveTextContent("Thursday works.");
    await userEvent.click(canvas.getByRole("button", { name: "Show quoted text" }));
    await expect(canvas.getByRole("textbox", { name: "Quoted text" })).toBeVisible();
    await userEvent.click(box);
    // ⌘↵ on Apple platforms, Ctrl+↵ elsewhere: exactly one of them sends.
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}{Control>}{Enter}{/Control}");
    await expect(args.onSend).toHaveBeenCalledOnce();
  },
};

/** To, Cc and Bcc suggest people as you type: ↓ and Enter add one, Esc closes the list. */
export const SuggestPeople: Story = {
  args: {
    title: "New message",
    to: [],
    quoted: undefined,
    subject: "",
    focus: "to",
    status: "",
    suggesting: true,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const to = canvas.getByRole("combobox", { name: "To" });
    await userEvent.type(to, "mar");
    await expect(canvas.getByRole("listbox", { name: "Suggestions for To" })).toBeVisible();
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await expect(canvas.getByText("Mário Lopes")).toBeVisible();
    await userEvent.click(canvas.getByRole("button", { name: "Cc Bcc" }));
    await userEvent.type(canvas.getByRole("combobox", { name: "Cc" }), "mar");
    const list = canvas.getByRole("listbox", { name: "Suggestions for Cc" });
    // Mário is in To already.
    await expect(within(list).getAllByRole("option")).toHaveLength(2);
  },
};
