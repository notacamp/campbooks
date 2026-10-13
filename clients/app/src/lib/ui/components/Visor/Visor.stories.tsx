import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";

import { ACCENTS } from "../../tokens/tokens";

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
/** Something went wrong on Campbooks' side. */
export const Dizzy: Story = { args: { state: "dizzy" } };
/** A big win: inbox zero, every ask answered. Rare by design. */
export const Starry: Story = { args: { state: "starry" } };
/** A warm moment: a thank-you, good news. */
export const Hearts: Story = { args: { state: "hearts" } };
/** Optional tint for the star and heart eyes; the eyes are white otherwise. */
export const Tinted: Story = {
  render: (args) => (
    <span className="inline-flex gap-4">
      <Visor {...args} state="starry" tinted />
      <Visor {...args} state="hearts" tinted />
    </span>
  ),
};

/** With `announce`, the state is read out ("Campbooks is reading"). */
export const Announced: Story = {
  args: { state: "reading", announce: true },
  play: async ({ canvasElement }) => {
    await expect(
      within(canvasElement).getByRole("img", { name: "Campbooks is reading" }),
    ).toBeVisible();
  },
};

const SIZES = ["sm", "md", "lg", "xl"] as const;

function StateTable({ tinted = false }: { tinted?: boolean }) {
  return (
    <table className="border-separate">
      <tbody>
        {VISOR_STATES.map((state) => (
          <tr key={state}>
            <th scope="row" className="pr-4 text-left">
              {state}
            </th>
            {SIZES.map((size) => (
              <td key={size} className="p-2">
                <Visor state={state} size={size} tinted={tinted} />
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Every state at every size. Motion stops under prefers-reduced-motion. */
export const Matrix: Story = { render: () => <StateTable /> };

/** The same grid with the star and heart tints on. */
export const MatrixTinted: Story = { render: () => <StateTable tinted /> };

/** Every state at every size under each accent; the toolbar's theme switch covers light and dark. */
export const Accents: Story = {
  render: () => (
    <div className="flex flex-col gap-6">
      {ACCENTS.map((accent) => (
        <div key={accent} data-accent={accent}>
          <StateTable />
        </div>
      ))}
    </div>
  ),
};

/** No eye may poke out of the glass, at any size or state (animation paused to measure the rest pose). */
export const Containment: Story = {
  render: () => (
    <>
      <style>{"[data-state] , [data-state] * { animation: none !important; }"}</style>
      <StateTable />
    </>
  ),
  play: async ({ canvasElement }) => {
    const visors = canvasElement.querySelectorAll<HTMLElement>("[data-state][data-size]");
    await expect(visors.length).toBe(VISOR_STATES.length * SIZES.length);
    for (const visor of visors) {
      const glass = visor.getBoundingClientRect();
      const eyes = visor.querySelectorAll<HTMLElement | SVGElement>(":scope > *");
      for (const eye of eyes) {
        const box = eye.getBoundingClientRect();
        const where = `${visor.dataset["state"]} ${visor.dataset["size"]}`;
        const margin = glass.height * 0.08;
        await expect(box.top, `${where} top`).toBeGreaterThanOrEqual(glass.top + margin);
        await expect(box.bottom, `${where} bottom`).toBeLessThanOrEqual(glass.bottom - margin);
        await expect(box.left, `${where} left`).toBeGreaterThanOrEqual(glass.left + margin);
        await expect(box.right, `${where} right`).toBeLessThanOrEqual(glass.right - margin);
      }
    }
  },
};
