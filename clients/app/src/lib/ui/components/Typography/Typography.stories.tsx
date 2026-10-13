import type { Meta, StoryObj } from "@storybook/react-vite";

import { Heading, Text } from "./Typography";

const meta = {
  title: "Components/Typography",
  component: Heading,
  args: { level: 1, size: "hero", children: "6 things need you" },
} satisfies Meta<typeof Heading>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Hero: Story = {};

export const Headings: Story = {
  render: () => (
    <div className="flex flex-col gap-4">
      <Heading level={1} size="hero">
        6 things need you
      </Heading>
      <Heading level={2} size="page">
        Q4 proposal: can you review it?
      </Heading>
      <Heading level={2} size="title">
        Nothing needs you
      </Heading>
      <Heading level={2} size="pane">
        Primary
      </Heading>
      <Heading level={3} size="section">
        Waiting on others
      </Heading>
    </div>
  ),
};

export const TextTones: Story = {
  render: () => (
    <div className="flex flex-col gap-2">
      <Text caps tone="accent">
        Monday 12 October
      </Text>
      <Text>Primary text: Sofia is waiting on your review.</Text>
      <Text tone="secondary">Secondary text: 48 messages sorted since Friday.</Text>
      <Text tone="tertiary" size="sm">
        Tertiary text: Sorted into Primary, 98% sure
      </Text>
      <Text tone="danger" weight="semibold" size="xs">
        Today
      </Text>
      <Text numeric size="lg" weight="semibold">
        €2,340.00
      </Text>
      <div className="w-sidebar">
        <Text truncate>Maria do Carmo Sant’Ana Albuquerque e Vasconcelos</Text>
      </div>
    </div>
  ),
};
