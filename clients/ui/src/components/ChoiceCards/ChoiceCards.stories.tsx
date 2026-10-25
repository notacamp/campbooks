import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";

import { ChoiceCards } from "./ChoiceCards";

const meta = {
  title: "Components/ChoiceCards",
  parameters: { layout: "padded" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Kinds({ initial }: { initial: "business" | "personal" | null }) {
  const [value, setValue] = useState(initial);
  return (
    <div className="max-w-measure">
      <ChoiceCards
        label="What is this workspace for?"
        value={value}
        onChange={setValue}
        options={[
          {
            value: "business",
            title: "Business",
            description: "Clients, suppliers, invoices and the team.",
          },
          {
            value: "personal",
            title: "Personal",
            description: "Your household, bills and appointments.",
          },
        ]}
      />
    </div>
  );
}

function Goals() {
  const [value, setValue] = useState<string | null>("guess");
  return (
    <div className="max-w-measure">
      <ChoiceCards
        label="What should happen to Design Notes’s mail?"
        value={value}
        onChange={setValue}
        options={[
          {
            value: "guess",
            title: "Archive them as they arrive",
            tag: "Scout’s guess",
            description: "They skip Home and leave the inbox the moment they come in.",
          },
          {
            value: "highlight",
            title: "Highlight them",
            tag: "I like these",
            description: "Scout makes sure you see each new one. Nothing is pinned.",
          },
          { value: "read", title: "Mark them read", description: "Nothing else changes." },
        ]}
      />
    </div>
  );
}

export const WithTags: Story = { render: () => <Goals /> };

export const NothingChosen: Story = { render: () => <Kinds initial={null} /> };

export const Chosen: Story = { render: () => <Kinds initial="personal" /> };
