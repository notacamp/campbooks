import type { Meta, StoryObj } from "@storybook/react-vite";

import { Tag } from "./Tag";

const meta = {
  title: "Components/Tag",
  component: Tag,
  args: { children: "2 messages" },
} satisfies Meta<typeof Tag>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const WithIcon: Story = {
  args: { icon: "attachment", children: "Brightloop-Q4-scope-v3.pdf · 2.4 MB" },
};

/** Long values are cut with an ellipsis inside their container. */
export const Long: Story = {
  render: () => (
    <div className="flex w-sidebar">
      <Tag icon="document">
        contrato-prestacao-servicos-contabilidade-2027-versao-revista-final.pdf
      </Tag>
    </div>
  ),
};
