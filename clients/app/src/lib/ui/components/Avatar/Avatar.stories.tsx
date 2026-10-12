import type { Meta, StoryObj } from "@storybook/react-vite";

import { Avatar } from "./Avatar";

const meta = {
  title: "Components/Avatar",
  component: Avatar,
  args: { name: "Sofia Pessoa" },
} satisfies Meta<typeof Avatar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Person: Story = {};

/** Machines and bulk senders are squircles. */
export const Service: Story = { args: { name: "Cloudhost", kind: "service" } };

export const Sizes: Story = {
  render: () => (
    <div className="flex items-center gap-3">
      <Avatar name="Rui Matos" size="sm" />
      <Avatar name="Rui Matos" size="md" />
      <Avatar name="Rui Matos" size="lg" />
      <Avatar name="Bankly" kind="service" size="sm" />
      <Avatar name="Bankly" kind="service" size="md" />
      <Avatar name="Bankly" kind="service" size="lg" />
    </div>
  ),
};

/** Eight low-chroma identity hues, picked by a hash of the name. */
export const Hues: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-3">
      {[
        "Sofia Pessoa",
        "Marta Duarte",
        "Rui Matos",
        "Joana Reis",
        "Tiago Mendes",
        "Ana Lopes",
        "Inês Carvalho",
        "Pedro Nunes",
        "Carla Sousa",
        "Hugo Alves",
      ].map((name) => (
        <Avatar key={name} name={name} label={name} />
      ))}
    </div>
  ),
};
