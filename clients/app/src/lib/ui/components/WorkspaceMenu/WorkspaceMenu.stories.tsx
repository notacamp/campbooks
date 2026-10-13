import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";

import { WorkspaceMenu, type WorkspaceMenuItem } from "./WorkspaceMenu";

const WORKSPACES: WorkspaceMenuItem[] = [
  { id: "w1", name: "Duarte & Pessoa", mark: "DP", kind: "Business", accent: "business" },
  { id: "w2", name: "Household", mark: "HO", kind: "Personal", accent: "personal" },
  { id: "w3", name: "Quinta do Vale", mark: "QV", kind: "Business", accent: "forest" },
];

function Demo() {
  const [open, setOpen] = useState(true);
  const [current, setCurrent] = useState("w1");
  return (
    <>
      <Button
        shortcut="workspace.switch"
        onClick={() => {
          setOpen(true);
        }}
      >
        Switch workspace
      </Button>
      <WorkspaceMenu
        open={open}
        onClose={() => {
          setOpen(false);
        }}
        workspaces={WORKSPACES}
        currentId={current}
        onSelect={setCurrent}
        onCreate={fn()}
      />
    </>
  );
}

const meta = {
  title: "Components/WorkspaceMenu",
  parameters: { layout: "fullscreen" },
  render: () => <Demo />,
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** Each workspace in its own accent; the current one is marked and focused. */
export const Open: Story = {};

/** ↓ moves to the next workspace. */
export const Keyboard: Story = {
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    await expect(body.getByRole("button", { name: /Duarte & Pessoa/u })).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    await expect(body.getByRole("button", { name: /Household/u })).toHaveFocus();
  },
};
