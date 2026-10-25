import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";

import { Select, TextField } from "./Field";

const meta = {
  title: "Components/Field",
  component: TextField,
  args: { label: "Workspace name", placeholder: "Duarte & Pessoa" },
} satisfies Meta<typeof TextField>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Text: Story = {};

export const Large: Story = {
  args: { size: "lg", hint: "You can use any name, up to 100 characters." },
};

export const WithError: Story = {
  args: {
    label: "Email",
    type: "email",
    defaultValue: "sofia@",
    error: "Enter a full email address, like sofia@example.com.",
  },
};

/** A page's title edited in place (a task's, and the new-task form's). */
export const Title: Story = {
  args: { size: "title", hideLabel: true, label: "Task title", placeholder: "What needs doing?" },
};

export const HiddenLabel: Story = { args: { hideLabel: true, label: "Search members" } };

export const Disabled: Story = { args: { disabled: true, defaultValue: "Household" } };

function RoleSelect({ disabled = false }: { disabled?: boolean }) {
  const [value, setValue] = useState<"member" | "admin">("member");
  return (
    <Select
      label="Role"
      value={value}
      onChange={setValue}
      disabled={disabled}
      options={[
        { value: "member", label: "Member" },
        { value: "admin", label: "Admin" },
      ]}
    />
  );
}

export const SelectField: Story = { render: () => <RoleSelect /> };

export const SelectDisabled: Story = { render: () => <RoleSelect disabled /> };
