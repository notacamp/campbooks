import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";

import { Avatar } from "../Avatar/Avatar";

import { CheckCards } from "./CheckCards";

const meta = {
  title: "Components/CheckCards",
  parameters: { layout: "padded" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const PEOPLE = [
  { value: "rita", name: "Rita Gomes", address: "rita@silva-contas.example", count: "41 emails" },
  {
    value: "duarte",
    name: "Duarte Pessoa",
    address: "duarte@pessoa-obras.example",
    count: "28 emails",
  },
  {
    value: "marta",
    name: "Marta Ferreira",
    address: "marta@ferreira-casa.example",
    count: "19 emails",
  },
] as const;

function People({ initial }: { initial: string[] }) {
  const [value, setValue] = useState(initial);
  return (
    <div className="max-w-measure">
      <CheckCards
        label="Whose mail should always reach you?"
        value={value}
        onChange={setValue}
        options={PEOPLE.map((person) => ({
          value: person.value,
          title: person.name,
          description: person.address,
          meta: person.count,
          leading: <Avatar name={person.name} />,
        }))}
      />
    </div>
  );
}

export const NothingChosen: Story = { render: () => <People initial={[]} /> };

export const TwoChosen: Story = { render: () => <People initial={["rita", "marta"]} /> };

export const TitlesOnly: Story = {
  render: function TitlesOnly() {
    const [value, setValue] = useState<string[]>(["lists"]);
    return (
      <div className="max-w-measure">
        <CheckCards
          label="What never needs you?"
          hideLabel
          value={value}
          onChange={setValue}
          options={[
            { value: "lists", title: "Newsletters and mailing lists" },
            { value: "automatic", title: "Automatic notifications" },
          ]}
        />
      </div>
    );
  },
};
