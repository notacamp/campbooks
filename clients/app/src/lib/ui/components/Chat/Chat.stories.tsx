import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";
import { ThinkingTrace } from "../ThinkingTrace/ThinkingTrace";

import {
  ChatAbout,
  ChatField,
  ChatMessage,
  ChatNotice,
  ChatPanel,
  ChatProposal,
  ChatRecent,
  ChatSteps,
  ChatSuggestions,
  ChatWorking,
} from "./Chat";

const ANSWER =
  "It is under Pay because Marta asks for November's rent, due on the 1st.\n\n" +
  "- Scout read it as a bill, 91% sure.\n- You have not said it is paid.";

const NOTE = "Scout asks before it changes anything.";

function Field({ busy = false }: { busy?: boolean }) {
  const [value, setValue] = useState("");
  return (
    <ChatField
      label="Ask about this"
      value={value}
      onChange={setValue}
      onAsk={fn()}
      askLabel="Ask"
      busy={busy}
      note={NOTE}
    />
  );
}

const about = (
  <ChatAbout
    label="About"
    action={
      <Button variant="link" onClick={fn()}>
        Open
      </Button>
    }
  >
    Marta Silva · Rent for November
  </ChatAbout>
);

const meta = {
  title: "Components/Chat",
  component: ChatPanel,
  args: {
    title: "Scout",
    closeLabel: "Close",
    onClose: fn(),
    modal: false,
    about,
    footer: <Field />,
    children: null,
  },
  parameters: { layout: "fullscreen" },
  decorators: [
    (Story) => (
      <div className="flex justify-end">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChatPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A new chat: Scout's opening line and questions to tap. */
export const Empty: Story = {
  args: {
    children: (
      <ChatSuggestions
        label="Questions to start from"
        intro="Ask me about Marta Silva's email, Rent for November."
        questions={[
          "Why is this here?",
          "What happened to this email?",
          "What else has Marta sent?",
        ]}
        onPick={fn()}
      />
    ),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("region", { name: "Scout" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await userEvent.click(canvas.getByRole("button", { name: /Close/ }));
    await expect(args.onClose).toHaveBeenCalled();
  },
};

/** A question and Scout's answer, with what it looked at. */
export const Answered: Story = {
  args: {
    children: (
      <>
        <ChatMessage from="member" label="You asked" text="Why is this under Pay?" />
        <ChatMessage
          from="scout"
          label="Scout answered"
          text={ANSWER}
          after={
            <ChatSteps
              summary="What I looked at"
              steps={[
                { text: "Read what happened to this email" },
                {
                  text: "Looked for conversations: 2 found",
                  records: [
                    { id: "c-1", label: "Marta Silva · Rent for October", onOpen: fn() },
                    { id: "c-2", label: "Marta Silva · Keys for the flat", onOpen: fn() },
                  ],
                },
              ]}
            />
          }
        />
      </>
    ),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByText("What I looked at"));
    await expect(canvas.getByRole("button", { name: /Keys for the flat/ })).toBeVisible();
  },
};

/** Scout at work: its thinking, a line per step, the field still usable. */
export const Writing: Story = {
  args: {
    footer: <Field busy />,
    children: (
      <>
        <ChatMessage from="member" label="You asked" text="What happened to this email?" />
        <ChatMessage
          from="scout"
          label="Scout is answering"
          writing
          text="It came in on Monday and Scout read it as"
          before={
            <>
              <ThinkingTrace
                label="Scout is thinking…"
                text="Marta's email asks for the rent. The history shows the Pay need."
                live
                summary="How Scout thought about it"
              />
              <ChatWorking
                lines={["Reading what happened to this email…", "Looking for conversations…"]}
              />
            </>
          }
        />
      </>
    ),
  },
};

/** An answer that failed, with Try again, and one cut short. */
export const Problems: Story = {
  args: {
    children: (
      <>
        <ChatMessage from="member" label="You asked" text="When is it due?" />
        <ChatMessage
          from="scout"
          label="Scout answered"
          text=""
          after={
            <ChatNotice visor="dizzy" action={<Button onClick={fn()}>Try again</Button>}>
              Scout couldn&apos;t answer that.
            </ChatNotice>
          }
        />
        <ChatMessage from="member" label="You asked" text="And the one before?" />
        <ChatMessage
          from="scout"
          label="Scout answered"
          text="October's rent was paid on the 2nd, and"
          after={<ChatNotice>This answer was cut short.</ChatNotice>}
        />
      </>
    ),
  },
};

/** Where Scout can't chat: said in place of the field. */
export const Unavailable: Story = {
  args: {
    footer: <ChatNotice visor="asleep">Scout can&apos;t chat on this server.</ChatNotice>,
    children: null,
  },
};

/** Recent, in the header: the member's chats by their first question; one opens. */
export const Recent: Story = {
  args: {
    actions: (
      <ChatRecent
        label="Recent"
        current="c-1"
        items={[
          {
            id: "c-1",
            text: "What am I waiting on?",
            time: "15:02",
            dateTime: "2026-10-13T15:02:00Z",
          },
          {
            id: "c-2",
            text: "Why is Marta’s walkthrough email under Reply, and does she need an answer today?",
            time: "Yesterday",
            dateTime: "2026-10-12T09:40:00Z",
          },
          {
            id: "c-3",
            text: "What has it changed?",
            time: "9 Oct",
            dateTime: "2026-10-09T17:05:00Z",
          },
        ]}
        empty="No chats yet. Ask Scout something to start one."
        loading="Loading your chats…"
        onPick={fn()}
      />
    ),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Recent" }));
    const menu = canvas.getByRole("dialog", { name: "Recent" });
    await expect(within(menu).getAllByRole("button")).toHaveLength(3);
  },
};

/** On a phone: over the whole screen, as a dialog. */
export const Phone: Story = {
  args: { modal: true },
  play: async () => {
    const dialog = within(document.body).getByRole("dialog", { name: "Scout" });
    await expect(within(dialog).getByRole("textbox", { name: "Ask about this" })).toHaveFocus();
  },
};

/** Scout's proposals under its answer: pending, done with Undo, a rule quoted, and how cards end. */
export const Proposals: Story = {
  args: {
    children: (
      <>
        <ChatMessage from="member" label="You asked" text="Can you get this out of my inbox?" />
        <ChatMessage
          from="scout"
          label="Scout answered"
          text="I proposed archiving it, and a rule for the next ones. Confirm on the cards."
          after={
            <>
              <ChatProposal
                label="Scout proposes"
                title="Archive"
                about="Marta Silva · Rent for November"
                state="pending"
                actions={
                  <>
                    <Button onClick={fn()}>Confirm</Button>
                    <Button onClick={fn()}>Not now</Button>
                  </>
                }
              />
              <ChatProposal
                label="Scout proposes"
                title="A rule for Teach Scout"
                quote="Keep newsletters from The Weekly off Home"
                quoteLabel="Scout’s words"
                state="pending"
                actions={
                  <>
                    <Button onClick={fn()}>Review the rule</Button>
                    <Button onClick={fn()}>Not now</Button>
                  </>
                }
              />
              <ChatProposal
                label="Scout proposes"
                title="Add the label"
                about="Marta Silva · Rent for November"
                quote="Accountant"
                quoteLabel="Scout’s words"
                state="done"
                status="Done."
                actions={<Button onClick={fn()}>Undo</Button>}
              />
              <ChatProposal
                label="Scout proposed"
                title="Mark done"
                about="Marta Silva · Rent for November"
                state="failed"
                status="This couldn’t be done: the need changed since."
              />
              <ChatProposal
                label="Scout proposed"
                title="Pin"
                about="Marta Silva · Rent for November"
                state="declined"
                status="Not now."
              />
            </>
          }
        />
      </>
    ),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("region", { name: "Archive" })).toBeVisible();
    await expect(canvas.getByText("Keep newsletters from The Weekly off Home")).toBeVisible();
  },
};
