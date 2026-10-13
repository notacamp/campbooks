import type { Meta, StoryObj } from "@storybook/react-vite";

import { Button } from "../Button/Button";
import { Visor } from "../Visor/Visor";

import { ActionBar } from "./ActionBar";

const meta = {
  title: "Components/ActionBar",
  component: ActionBar,
  args: {
    label: "Conversation actions",
    children: (
      <>
        <Button variant="primary" size="lg" icon="reply" grow>
          Reply
        </Button>
        <Button size="lg" leading={<Visor />} aria-label="Draft a reply for me">
          Draft
        </Button>
        <Button size="lg">Done</Button>
      </>
    ),
  },
  decorators: [
    (Story) => (
      <div className="max-w-list">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ActionBar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
