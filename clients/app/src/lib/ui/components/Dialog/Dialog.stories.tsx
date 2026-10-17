import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";

import { Dialog, DialogChoices, type DialogResult } from "./Dialog";

const TIMES = [
  { id: "today", label: "Later today", detail: "today 18:00" },
  { id: "tomorrow", label: "Tomorrow morning", detail: "tomorrow 08:00" },
  { id: "monday", label: "Monday morning", detail: "Monday 08:00" },
  { id: "week", label: "Next week", detail: "Fri 23 Oct 08:00" },
];

const meta = {
  title: "Components/Dialog",
  component: Dialog,
  args: {
    open: true,
    onClose: fn(),
    title: "Snooze Marta Ferreira until…",
    description: "It leaves Home until then, and comes back with its sentence.",
  },
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Dialog>;

export default meta;
type Story = StoryObj<typeof meta>;

/** One decision: choices with their number keys. Esc closes with nothing changed. */
export const Choices: Story = {
  args: {
    children: <DialogChoices label="When" choices={TIMES} onChoose={fn()} />,
  },
  play: async () => {
    const page = within(document.body);
    await expect(page.getByRole("button", { name: /Later today/ })).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    await expect(page.getByRole("button", { name: /Tomorrow morning/ })).toHaveFocus();
  },
};

/** A choice that runs the AI carries the visor. */
export const AiChoice: Story = {
  args: {
    title: "Reply to Marta",
    description: "What should it say? Pick one.",
    children: (
      <DialogChoices
        label="Answers"
        onChoose={fn()}
        choices={[
          { id: "yes", label: "Yes, Thursday 10:00 works", ai: true },
          { id: "move", label: "Ask to move it to the afternoon", ai: true },
        ]}
      />
    ),
  },
};

/** Body and footer, no visor (the smart-rule confirmation shape). */
export const WithFooter: Story = {
  args: {
    title: "A rule for EDP invoices",
    description: "This is saved as a smart rule. Check it before it starts working.",
    visor: null,
    children: (
      <p>
        When an invoice from EDP Comercial arrives, file it in Paper and remind me 2 days before the
        debit.
      </p>
    ),
    footer: (
      <>
        <Button variant="primary" shortcut="compose.send">
          Save smart rule
        </Button>
        <Button>Cancel</Button>
      </>
    ),
  },
};

/** The result: visor happy, a sentence and a draining bar; closes by itself or on any key. */
export const Result: Story = {
  args: {
    title: "Reply to Marta",
    result: { title: "Sending to Marta", detail: "Undo works for 10 seconds after sending." },
    onResultEnd: fn(),
  },
  play: async ({ args }) => {
    await expect(within(document.body).getByRole("status")).toHaveTextContent("Sending to Marta");
    await userEvent.keyboard("a");
    await expect(args.onResultEnd).toHaveBeenCalled();
  },
};

function Flow() {
  const [open, setOpen] = useState(false);
  const [result, setResult] = useState<DialogResult | undefined>();
  return (
    <div className="p-6">
      <Button
        shortcut="item.snooze"
        onClick={() => {
          setOpen(true);
        }}
      >
        Snooze…
      </Button>
      <Dialog
        open={open}
        onClose={() => {
          setOpen(false);
        }}
        title="Snooze Marta Ferreira until…"
        result={result}
        onResultEnd={() => {
          setOpen(false);
          setResult(undefined);
        }}
      >
        <DialogChoices
          label="When"
          choices={TIMES}
          onChoose={(id) => {
            setResult({ title: `Snoozed until ${TIMES.find((t) => t.id === id)?.detail ?? ""}` });
          }}
        />
      </Dialog>
    </div>
  );
}

/** Open, pick with a number key, see the result, back where you were. */
export const Interactive: StoryObj = {
  render: () => <Flow />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: /Snooze/ }));
    await userEvent.keyboard("2");
    await expect(within(document.body).getByRole("status")).toHaveTextContent(
      "Snoozed until tomorrow 08:00",
    );
  },
};
