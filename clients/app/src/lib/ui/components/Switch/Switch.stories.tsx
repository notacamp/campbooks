import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";

import { Switch, type SwitchProps } from "./Switch";

/** A switch that keeps its own state, so the story can be pressed. */
function Pressable(props: SwitchProps) {
  const [checked, setChecked] = useState(props.checked);
  return <Switch {...props} checked={checked} onChange={setChecked} />;
}

const meta = {
  title: "Components/Switch",
  component: Switch,
  args: {
    label: "Show Work",
    checked: true,
    onChange: () => {
      // The story only shows the control.
    },
  },
  render: (args) => <Pressable {...args} />,
} satisfies Meta<typeof Switch>;

export default meta;
type Story = StoryObj<typeof meta>;

export const On: Story = {};
export const Off: Story = { args: { checked: false } };
export const Disabled: Story = { args: { disabled: true } };
