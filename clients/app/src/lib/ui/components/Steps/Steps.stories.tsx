import type { Meta, StoryObj } from "@storybook/react-vite";

import { Steps } from "./Steps";

const meta = {
  title: "Components/Steps",
  component: Steps,
  args: {
    label: "What happens next",
    steps: [
      "Your account is created, and you sign in with this Google account.",
      "You name your first workspace.",
      "You choose whether to connect your mail to it.",
    ],
  },
} satisfies Meta<typeof Steps>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ThreeSteps: Story = {};

export const OneStep: Story = { args: { steps: ["You name your first workspace."] } };
