import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

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
