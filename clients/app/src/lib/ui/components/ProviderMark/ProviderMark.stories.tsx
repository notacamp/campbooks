import type { Meta, StoryObj } from "@storybook/react-vite";

import { ProviderMark } from "./ProviderMark";

const meta = {
  title: "Components/ProviderMark",
  component: ProviderMark,
  args: { name: "Gmail" },
} satisfies Meta<typeof ProviderMark>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Medium: Story = {};
export const Large: Story = { args: { size: "lg" } };

/** Every provider a mailbox can come from, next to its name. */
export const Providers: Story = {
  render: () => (
    <ul className="flex flex-col gap-2">
      {["Gmail", "Zoho Mail", "Microsoft 365"].map((name) => (
        <li key={name} className="flex items-center gap-2">
          <ProviderMark name={name} />
          {name}
        </li>
      ))}
    </ul>
  ),
};
