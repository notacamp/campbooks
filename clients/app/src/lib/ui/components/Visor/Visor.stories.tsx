import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";

import { Visor, VISOR_STATES } from "./Visor";

const meta = {
  title: "Components/Visor",
  component: Visor,
  args: { state: "idle", size: "xl" },
  argTypes: {
    state: { control: "select", options: VISOR_STATES },
    size: { control: "inline-radio", options: ["sm", "md", "lg", "xl"] },
  },
} satisfies Meta<typeof Visor>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Idle: Story = {};
export const Reading: Story = { args: { state: "reading" } };
export const Thinking: Story = { args: { state: "thinking" } };
export const Unsure: Story = { args: { state: "unsure" } };
export const Happy: Story = { args: { state: "happy" } };
export const Alert: Story = { args: { state: "alert" } };
export const Asleep: Story = { args: { state: "asleep" } };

/** With `announce`, the state is read out ("Campbooks is reading"). */
export const Announced: Story = {
  args: { state: "reading", announce: true },
  play: async ({ canvasElement }) => {
    await expect(
      within(canvasElement).getByRole("img", { name: "Campbooks is reading" }),
    ).toBeVisible();
  },
};

/** Every state at every size. Motion stops under prefers-reduced-motion. */
export const Matrix: Story = {
  render: () => (
    <table className="border-separate">
      <tbody>
        {VISOR_STATES.map((state) => (
          <tr key={state}>
            <th scope="row" className="pr-4 text-left">
              {state}
            </th>
            {(["sm", "md", "lg", "xl"] as const).map((size) => (
              <td key={size} className="p-2">
                <Visor state={state} size={size} />
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  ),
};
