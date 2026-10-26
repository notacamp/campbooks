import type { Meta, StoryObj } from "@storybook/react-vite";

import { Fact, FactList } from "./Facts";

const meta = {
  title: "Components/Facts",
  parameters: { layout: "padded" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** Plain facts, a muted detail, and every status. */
export const Statuses: Story = {
  render: () => (
    <div className="max-w-measure">
      <FactList>
        <Fact label="Version">0.1.0</Fact>
        <Fact label="Database" status="ok" detail="3 ms · 18 migrations">
          Reachable
        </Fact>
        <Fact label="Decide" status="problem" detail="qwen3:8b · http://ollama:11434">
          Not answering
        </Fact>
        <Fact label="Vision" status="off">
          Not configured
        </Fact>
      </FactList>
    </div>
  ),
};

/** Long values wrap instead of overflowing on a phone. */
export const LongValues: Story = {
  render: () => (
    <div className="max-w-sidebar">
      <FactList>
        <Fact
          label="Public address"
          detail="https://a-very-long-subdomain.of-an-instance.example.com"
        >
          https://campbooks.with-a-rather-long-host-name.example.com:8443
        </Fact>
        <Fact label="Model">convaiinnovations/laya-multilingual-decide-large</Fact>
      </FactList>
    </div>
  ),
};
