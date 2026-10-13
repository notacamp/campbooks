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

export const NothingChosen: Story = { render: () => <Kinds initial={null} /> };

export const Chosen: Story = { render: () => <Kinds initial="personal" /> };
