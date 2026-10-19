import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";

import { DEFAULT_SHORTCUTS } from "~/lib/shortcuts";

import { Kbd, KbdHint } from "./Kbd";

const meta = {
  title: "Components/Kbd",
  component: KbdHint,
} satisfies Meta<typeof KbdHint>;

export default meta;
type Story = StoryObj<typeof meta>;

export const SingleKey: Story = { render: () => <Kbd>E</Kbd> };

/** Keys are read from the shortcut registry, so the hint matches the binding. */
export const FromRegistry: Story = { args: { shortcut: "item.done" } };

/** ⌘K on Apple platforms, Ctrl K elsewhere. */
export const WithModifier: Story = { args: { shortcut: "command.open" } };

export const Sequence: Story = { args: { shortcut: "nav.inbox" } };

/** The command menu writes "then" between the steps of a sequence. */
export const SequenceWithThen: Story = { args: { shortcut: "nav.inbox", withThen: true } };

/** In the shortcut sheet the keys are read out to screen readers. */
export const Announced: Story = {
  args: { shortcut: "nav.inbox", announce: true },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByText("Shortcut: G then I")).toBeInTheDocument();
  },
};

export const OneOffKeys: Story = { args: { keys: "1" } };

/** Every default binding. */
export const AllDefaults: Story = {
  render: () => (
    <dl className="grid grid-cols-1 gap-2 sm:grid-cols-2">
      {DEFAULT_SHORTCUTS.map((definition) => (
        <div key={definition.id} className="flex items-center justify-between gap-3">
          <dt>{definition.id}</dt>
          <dd>
            <KbdHint shortcut={definition.id} />
          </dd>
        </div>
      ))}
    </dl>
  ),
};
