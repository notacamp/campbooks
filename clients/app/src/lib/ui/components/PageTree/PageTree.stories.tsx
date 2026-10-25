import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "@notacamp/ui";

import { PageTree, PageTreeGroup, PageTreeRow } from "./PageTree";

const meta = {
  title: "Components/PageTree",
  component: PageTree,
  args: { label: "Pages", children: null },
  decorators: [
    (Story) => (
      <div className="w-full max-w-measure">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PageTree>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <PageTree {...args}>
      <PageTreeGroup name="Workspace" action={<Button variant="ghost">New page</Button>}>
        <PageTreeRow label="Client onboarding" icon="tasks" current onOpen={fn()} onAdd={fn()} />
        <PageTreeRow
          label="Team handbook"
          icon="people"
          expanded
          toggleLabel="Hide sub-pages of Team handbook"
          addLabel="Add a sub-page to Team handbook"
          onToggle={fn()}
          onOpen={fn()}
          onAdd={fn()}
        />
        <PageTreeRow label="Holidays" depth={1} onOpen={fn()} onAdd={fn()} />
        <PageTreeRow label="Expenses" depth={1} icon="money" onOpen={fn()} onAdd={fn()} />
      </PageTreeGroup>
      <PageTreeGroup name="Private">
        <PageTreeRow
          label="Weekly plan"
          icon="schedule"
          expanded={false}
          toggleLabel="Show sub-pages of Weekly plan"
          onToggle={fn()}
          onOpen={fn()}
        />
      </PageTreeGroup>
      <PageTreeGroup name="Shared with you" empty="Nothing is shared with you yet." />
    </PageTree>
  ),
};

/** Where a dragged page would land: before, inside or after a row. */
export const DropTargets: Story = {
  render: (args) => (
    <PageTree {...args}>
      <PageTreeGroup name="Workspace">
        <PageTreeRow label="Lands before this one" dropZone="before" onOpen={fn()} />
        <PageTreeRow label="Lands inside this one" dropZone="inside" onOpen={fn()} />
        <PageTreeRow label="Lands after this one" dropZone="after" onOpen={fn()} />
        <PageTreeRow label="Being dragged" dragging draggable onOpen={fn()} />
      </PageTreeGroup>
    </PageTree>
  ),
};

/** A long title is cut on one line. */
export const LongTitle: Story = {
  render: (args) => (
    <PageTree {...args}>
      <PageTreeGroup name="Workspace">
        <PageTreeRow
          label="The handbook for the whole team, with every policy, form and holiday rule in one page"
          onOpen={fn()}
          onAdd={fn()}
        />
      </PageTreeGroup>
    </PageTree>
  ),
};

/** The toggle, the title and the add button are separate buttons. */
export const Interaction: Story = {
  render: (args) => (
    <PageTree {...args}>
      <PageTreeGroup name="Workspace">
        <PageTreeRow
          label="Team handbook"
          expanded={false}
          toggleLabel="Show sub-pages of Team handbook"
          addLabel="Add a sub-page to Team handbook"
          onToggle={fn()}
          onOpen={fn()}
          onAdd={fn()}
        />
      </PageTreeGroup>
    </PageTree>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const toggle = canvas.getByRole("button", { name: "Show sub-pages of Team handbook" });
    await userEvent.click(toggle);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(canvas.getByRole("button", { name: "Team handbook" }));
    await userEvent.click(canvas.getByRole("button", { name: "Add a sub-page to Team handbook" }));
  },
};
