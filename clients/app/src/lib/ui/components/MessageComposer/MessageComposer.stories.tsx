import type { Meta, StoryObj } from "@storybook/react-vite";
import type { Editor } from "@tiptap/core";
import { useState } from "react";
import { expect, fireEvent, fn, userEvent, within } from "storybook/test";

import type { Recipient, RecipientSuggestion } from "../RecipientField/RecipientField";

import {
  MessageComposer,
  type ComposerAttachment,
  type ComposerFrom,
  type ComposerModes,
  type ComposerScout,
  type ComposerSignature,
  type ComposerUpload,
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
  uploading?: ComposerUpload[];
  attachmentBudget?: number;
  onAttach?: (files: File[]) => void;
  onSendLater?: (at: Date) => void;
  signature?: ComposerSignature;
  fromPicker?: ComposerFrom;
  modes?: ComposerModes;
  scout?: ComposerScout;
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
    // Typed, then selected as a drag would (the editor sits on its element;
    // userEvent's typing drops keys in a ProseMirror editor).
    const editor = (box as HTMLElement & { editor: Editor }).editor;
    editor.chain().focus().insertContent("Thursday works.").selectAll().run();
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

const MB = 1024 * 1024;

/** A reply with files from the device: the type tile, the name, the size, Remove; the total under them. */
export const WithAttachments: Story = {
  args: {
    body: "The signed lease and the parking form are attached.",
    attachmentBudget: 25 * MB,
    onAttach: fn(),
    attachments: [
      { id: "u1", filename: "lease-4b-signed.pdf", size: 412_000, source: "uploaded" },
      {
        id: "u2",
        filename: "parking-form-P2-14-with-a-very-long-name-that-gets-cut.docx",
        size: 38_400,
        source: "uploaded",
      },
      { id: "u3", filename: "entrance.jpeg", size: 2_900_000, source: "uploaded" },
    ],
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("3.2 MB of 25 MB")).toBeVisible();
    const file = new File(["%PDF"], "receipt.pdf", { type: "application/pdf" });
    await userEvent.upload(canvas.getByTestId("composer-files"), file);
    await expect(args.onAttach).toHaveBeenCalledWith([file]);
    await userEvent.click(canvas.getByRole("button", { name: "Remove entrance.jpeg" }));
    await expect(canvas.queryByText("entrance.jpeg")).toBeNull();
  },
};

/** A file on its way: "Uploading…" with no Remove, counted in the total. */
export const Uploading: Story = {
  args: {
    body: "Here is the photo.",
    attachmentBudget: 25 * MB,
    onAttach: fn(),
    attachments: [{ id: "u1", filename: "lease-4b-signed.pdf", size: 412_000, source: "uploaded" }],
    uploading: [{ key: 1, filename: "boiler-photo.heic", size: 6 * MB }],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const busy = canvas.getByText("boiler-photo.heic").closest("li");
    await expect(busy).toHaveAttribute("aria-busy", "true");
    await expect(within(busy as HTMLElement).getByText("Uploading…")).toBeVisible();
  },
};

/** Close to the 25 MB a message may carry. */
export const NearTheLimit: Story = {
  args: {
    body: "The full scan is attached.",
    attachmentBudget: 25 * MB,
    onAttach: fn(),
    attachments: [
      { id: "u1", filename: "scan-part-1.pdf", size: 12 * MB, source: "uploaded" },
      { id: "u2", filename: "scan-part-2.pdf", size: 12.4 * MB, source: "uploaded" },
    ],
  },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByText("24.4 MB of 25 MB")).toBeVisible();
  },
};

const SETTINGS = "/w/duarte/settings/mailboxes";

/** The signature under the text, muted and read-only: Leave off, or Edit in Settings. */
export const WithSignature: Story = {
  args: {
    body: "Thursday works for me.",
    signature: {
      text: "**Sam Rivera**\nRivera Lettings · [rivera.example](https://rivera.example)",
      available: true,
      onLeaveOff: fn(),
      onAdd: fn(),
      settingsHref: SETTINGS,
    },
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Sam Rivera")).toBeVisible();
    await userEvent.click(canvas.getByRole("button", { name: "Leave off" }));
    await expect(args.signature?.onLeaveOff).toHaveBeenCalledOnce();
    await expect(canvas.getByRole("link", { name: "Edit in Settings" })).toBeVisible();
  },
};

/** Left off, with one to add back. */
export const SignatureLeftOff: Story = {
  args: {
    body: "Thursday works for me.",
    signature: {
      text: "",
      available: true,
      onLeaveOff: fn(),
      onAdd: fn(),
      settingsHref: SETTINGS,
    },
  },
  play: async ({ canvasElement, args }) => {
    await userEvent.click(within(canvasElement).getByRole("button", { name: "Add signature" }));
    await expect(args.signature?.onAdd).toHaveBeenCalledOnce();
  },
};

/** No signature for this mailbox yet: a way to Settings to make one. */
export const NoSignature: Story = {
  args: {
    body: "Thursday works for me.",
    signature: {
      text: "",
      available: false,
      onLeaveOff: fn(),
      onAdd: fn(),
      settingsHref: SETTINGS,
    },
  },
  play: async ({ canvasElement }) => {
    await expect(
      within(canvasElement).getByRole("link", { name: "Add a signature in Settings" }),
    ).toBeVisible();
  },
};

const TWO_INBOXES = [
  { id: "i1", label: "Sam Rivera <sam@example.com>" },
  { id: "i2", label: "Rivera Lettings <office@rivera-lettings.example>" },
];

/** From as a picker when the member may send from more than one inbox. */
export const FromPicker: Story = {
  args: {
    body: "Thursday works for me.",
    fromPicker: { options: TWO_INBOXES, value: "i1", onChange: fn() },
  },
  play: async ({ canvasElement, args }) => {
    const from = within(canvasElement).getByRole("combobox", { name: "From" });
    await userEvent.selectOptions(from, "i2");
    await expect(args.fromPicker?.onChange).toHaveBeenCalledWith("i2");
  },
};

/** Reply ▾ in the title: Reply, Reply all, Forward (R, ⇧R, F inside the menu). */
export const ReplyTypeMenu: Story = {
  args: { body: "Thursday works for me.", modes: { value: "reply", onChange: fn() } },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Change reply type" }));
    await userEvent.click(canvas.getByRole("menuitemradio", { name: /Forward/u }));
    await expect(args.modes?.onChange).toHaveBeenCalledWith("forward");
  },
};

/** Scout ▾ (⌘J): Shorter, Warmer, Fix spelling and Change…. */
export const ScoutMenu: Story = {
  args: {
    body: "hi maya thursday works for me ok",
    scout: { thinking: "", onRewrite: () => Promise.resolve("Hi Maya, Thursday works for me.") },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Ask Scout to rewrite" }));
    await expect(canvas.getByRole("button", { name: "Shorter" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Warmer" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Fix spelling" })).toBeVisible();
    await expect(canvas.getByRole("textbox", { name: "Change…" })).toBeVisible();
    await userEvent.click(canvas.getByRole("button", { name: "Fix spelling" }));
    await expect(await canvas.findByText("Hi Maya, Thursday works for me.")).toBeVisible();
  },
};

/** A play-less twin for recordings: pick Shorter and Scout keeps writing (the text read-only, its thinking showing). */
export const ScoutWriting: Story = {
  args: {
    body: "hi maya thursday works for me ok",
    scout: {
      thinking: "Reading the email. Keeping what the draft says.",
      onRewrite: () => new Promise<string | undefined>(() => null),
    },
  },
};

/** The chevron beside Send: tomorrow morning, the next working day and a time of one's own. */
export const SendLaterMenu: Story = {
  args: { body: "Thursday works for me.", onSendLater: fn() },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Send later" }));
    await expect(canvas.getByRole("button", { name: /Tomorrow morning/u })).toBeVisible();
    await expect(canvas.getByRole("button", { name: /Next working day/u })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Pick a time…" })).toBeVisible();
    await userEvent.click(canvas.getByRole("button", { name: /Tomorrow morning/u }));
    await expect(args.onSendLater).toHaveBeenCalledOnce();
  },
};

/** "Pick a time…": a date and a time, tomorrow at 08:00 to begin with. */
export const PickATime: Story = {
  args: { body: "Thursday works for me.", onSendLater: fn() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Send later" }));
    await userEvent.click(canvas.getByRole("button", { name: "Pick a time…" }));
    await expect(canvas.getByLabelText("Date")).toBeVisible();
    await expect(canvas.getByLabelText("Time")).toHaveValue("08:00");
  },
};

/** A time that has passed is refused, and the form says why. */
export const InvalidPickedTime: Story = {
  args: { body: "Thursday works for me.", onSendLater: fn() },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Send later" }));
    await userEvent.click(canvas.getByRole("button", { name: "Pick a time…" }));
    await fireEvent.change(canvas.getByLabelText("Date"), { target: { value: "2020-01-01" } });
    await userEvent.click(canvas.getByRole("button", { name: "Schedule" }));
    await expect(canvas.getByRole("alert")).toHaveTextContent("That time has passed");
    await expect(args.onSendLater).not.toHaveBeenCalled();
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
