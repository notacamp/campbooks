import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { Button } from "../Button/Button";
import { Dialog, DialogChoices } from "../Dialog/Dialog";

import { AskLine, ReplyDraft, ReplyQuote } from "./ReplyDraft";

const DRAFT =
  "Hi Marta,\n\nThursday at 10:00 works for me. See you at the site.\n\nBest,\nSam Rivera";

const meta = {
  title: "Components/ReplyDraft",
  component: ReplyDraft,
  args: {
    label: "Your reply",
    value: DRAFT,
    onChange: fn(),
    tools: (
      <>
        <Button ai>Shorter</Button>
        <Button variant="ghost">Back to the choices</Button>
      </>
    ),
  },
  parameters: { layout: "padded" },
} satisfies Meta<typeof ReplyDraft>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Scout's draft, editable, with the buttons that rework it. */
export const Draft: Story = {};

/** While Scout rewrites it: the text stays, read-only, and says what is happening. */
export const Rewriting: Story = {
  args: { busy: true, busyLabel: "Making it shorter…" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("textbox", { name: "Your reply" })).toHaveAttribute("readonly");
    await expect(canvas.getByRole("status")).toHaveTextContent("Making it shorter…");
  },
};

/** The words being answered, quoted (four lines at most). */
export const Quote: StoryObj = {
  render: () => (
    <ReplyQuote
      from="Marta Ferreira, Mon 12 Oct"
      text="Could you confirm Thursday at 10:00 works for the site visit? The contractor needs to know by Wednesday."
    />
  ),
};

/** Ask in your own words: the button carries the visor and stays off while the field is empty. */
export const Ask: StoryObj = {
  render: () => (
    <AskLine
      label="Or say it in your words"
      placeholder="e.g. yes, but I'll be 15 minutes late"
      action="Draft it"
      onSubmit={fn()}
    />
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: /Draft it/ })).toBeDisabled();
    await userEvent.type(canvas.getByRole("textbox"), "yes, but later");
    await expect(canvas.getByRole("button", { name: /Draft it/ })).toBeEnabled();
  },
};

/** Asking is off while Scout writes. */
export const AskDisabled: StoryObj = {
  render: () => <AskLine label="Ask for a change" action="Change" disabled onSubmit={fn()} />,
};

function ReplyFlow() {
  const [text, setText] = useState<string | undefined>();
  return (
    <Dialog
      open
      onClose={fn()}
      title="Reply to Marta"
      description={
        text === undefined
          ? "What should it say? Pick one, or say it in your words."
          : "Scout's draft. Edit it here, or ask for a change."
      }
      footer={
        text === undefined ? undefined : (
          <>
            <Button variant="primary" shortcut="compose.send">
              Send
            </Button>
            <Button>Open in composer</Button>
          </>
        )
      }
    >
      <ReplyQuote
        from="Marta Ferreira, Mon 12 Oct"
        text="Could you confirm Thursday at 10:00 works for the site visit?"
      />
      {text === undefined ? (
        <>
          <DialogChoices
            label="Likely replies"
            onChoose={() => {
              setText(DRAFT);
            }}
            choices={[
              { id: "yes", label: "Yes, Thursday 10:00 works", ai: true },
              { id: "move", label: "Ask to move it to the afternoon", ai: true },
            ]}
          />
          <AskLine
            label="Or say it in your words"
            action="Draft it"
            onSubmit={() => {
              setText(DRAFT);
            }}
          />
        </>
      ) : (
        <>
          <ReplyDraft
            label="Your reply"
            value={text}
            onChange={setText}
            tools={<Button ai>Shorter</Button>}
          />
          <AskLine label="Ask for a change" action="Change" onSubmit={fn()} />
        </>
      )}
    </Dialog>
  );
}

/** In the reply dialog: pick a likely reply, then edit the draft. */
export const InTheDialog: StoryObj = {
  parameters: { layout: "fullscreen" },
  render: () => <ReplyFlow />,
  play: async () => {
    const page = within(document.body);
    await userEvent.keyboard("1");
    await waitFor(async () => {
      await expect(page.getByRole("textbox", { name: "Your reply" })).toHaveFocus();
    });
  },
};
