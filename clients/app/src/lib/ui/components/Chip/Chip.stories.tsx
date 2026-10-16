import type { Meta, StoryObj } from "@storybook/react-vite";

import { Chip, ChipLink, ChipRow } from "./Chip";

const meta = {
  title: "Components/Chip",
  component: Chip,
  args: { children: "Primary" },
} satisfies Meta<typeof Chip>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Pressed: Story = { args: { pressed: true } };

/** A scrolling row of 44px filter chips, for phones. */
export const Row: Story = {
  render: () => (
    <ChipRow label="Streams">
      <Chip pressed>Primary</Chip>
      <Chip pressed={false}>Updates</Chip>
      <Chip pressed={false}>Receipts</Chip>
      <Chip pressed={false}>Newsletters</Chip>
      <Chip pressed={false}>Clients</Chip>
    </ChipRow>
  ),
};

/** Chips that go somewhere: links, the current one marked as the page. */
export const Links: Story = {
  render: () => (
    <nav aria-label="Settings">
      <ChipRow label="Settings sections">
        <ChipLink href="#members">Members</ChipLink>
        <ChipLink href="#mailboxes" current>
          Mailboxes
        </ChipLink>
        <ChipLink href="#sign-in">Ways to sign in</ChipLink>
      </ChipRow>
    </nav>
  ),
};
