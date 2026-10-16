import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { Button } from "../Button/Button";
import { Line } from "../Queue/Queue";

import { Toast, UndoProvider, useUndoToast } from "./Toast";

const meta = {
  title: "Components/Toast",
  component: Toast,
  args: { children: "Done · Sofia Pessoa" },
} satisfies Meta<typeof Toast>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    action: (
      <Button variant="primary" shortcut="undo">
        Undo
      </Button>
    ),
  },
};

/** A failure that needs you: wraps, uses the alert visor, announced assertively. */
export const Alert: Story = {
  args: {
    tone: "alert",
    children: (
      <>
        <b>Not sent to Sofia.</b> The attachment is over the 25 MB limit of her mail server. Your
        draft is safe.
      </>
    ),
  },
};

/** An action that takes a moment: the visor thinks; Undo while it still can be taken back. */
export const Pending: Story = {
  args: {
    tone: "pending",
    children: "Sending to Marta…",
    action: (
      <Button variant="primary" shortcut="undo">
        Undo
      </Button>
    ),
  },
};

/** A failure stays until Retry or dismiss. */
export const Failed: Story = {
  args: {
    tone: "alert",
    children: "Couldn’t send to Marta. Your draft is kept.",
    action: <Button variant="primary">Retry</Button>,
    onDismiss: fn(),
  },
  play: async ({ canvasElement, args }) => {
    await userEvent.click(within(canvasElement).getByRole("button", { name: "Dismiss" }));
    await expect(args.onDismiss).toHaveBeenCalledOnce();
  },
};

function PendingDemo() {
  const undo = useUndoToast();
  return (
    <div className="flex flex-wrap gap-2">
      <Button
        onClick={() => {
          const handle = undo.pending({ label: "Sending to Marta…", cancel: fn() });
          setTimeout(() => {
            handle.done({ label: "Sent to Marta", undo: fn() });
          }, 1500);
        }}
      >
        Send (works)
      </Button>
      <Button
        onClick={() => {
          const handle = undo.pending({ label: "Sending to Marta…" });
          setTimeout(() => {
            handle.fail({
              label: "Couldn’t send to Marta. Your draft is kept.",
              retry: fn(),
            });
          }, 800);
        }}
      >
        Send (fails)
      </Button>
    </div>
  );
}

/** Pending, then done (with Undo) or failed (stays, with Retry). */
export const PendingThenOutcome: StoryObj = {
  render: () => (
    <UndoProvider>
      <PendingDemo />
    </UndoProvider>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Send (fails)" }));
    await waitFor(async () => {
      await expect(canvas.getByRole("alert")).toHaveTextContent("Couldn’t send to Marta.");
    });
  },
};

function UndoDemo() {
  const undo = useUndoToast();
  const [done, setDone] = useState<string[]>([]);
  const act = (who: string, label: string) => {
    setDone((current) => [...current, who]);
    undo.push({
      label: `${label} · ${who}`,
      undo: () => {
        setDone((current) => current.filter((name) => name !== who));
      },
      redo: () => {
        setDone((current) => [...current, who]);
      },
    });
  };
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap gap-2">
        {["Sofia Pessoa", "Rui Matos", "Cloudhost"].map((who) => (
          <Button
            key={who}
            disabled={done.includes(who)}
            onClick={() => {
              act(who, "Done");
            }}
          >
            Done · {who}
          </Button>
        ))}
      </div>
      <div className="flex flex-col">
        {undo.history.map((entry) => (
          <Line
            key={entry.id}
            undone={entry.undone}
            trailing={
              <Button
                variant="link"
                onClick={() => {
                  undo.toggle(entry.id);
                }}
              >
                {entry.undone ? "Redo" : "Undo"}
              </Button>
            }
          >
            {entry.label}
          </Line>
        ))}
      </div>
    </div>
  );
}

/** Every action is undoable: the toast offers Undo, Z undoes the latest, the history keeps the rest. */
export const UndoStack: Story = {
  render: () => (
    <UndoProvider>
      <UndoDemo />
    </UndoProvider>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Done · Sofia Pessoa" }));
    await userEvent.click(canvas.getByRole("button", { name: "Done · Rui Matos" }));
    const status = within(canvasElement.ownerDocument.body).getByRole("status", {
      name: "Notifications",
    });
    await expect(status).toHaveTextContent("Done · Rui Matos");
    await userEvent.keyboard("z");
    await waitFor(() => expect(status).toHaveTextContent("Undone: Done · Rui Matos"));
    await expect(canvas.getByRole("button", { name: "Done · Rui Matos" })).toBeEnabled();
  },
};
