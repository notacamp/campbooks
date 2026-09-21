/**
 * Visor stories — CSF3 format (Storybook 10).
 *
 * Shows every state, a range of sizes, and the gaze variant.
 * @storybook/react is not in package.json; CSF3 metadata is untyped objects
 * so the file compiles without that dep installed.
 */
import { type JSX } from "react";
import { Visor, type VisorProps } from "./visor";

// ── CSF3 metadata ─────────────────────────────────────────────────────────────

const meta = {
  title: "ui/Visor",
  component: Visor,
  tags: ["autodocs"],
  parameters: { layout: "centered" },
};

export default meta;

// ── Story type ────────────────────────────────────────────────────────────────

type Story = {
  args?: Partial<VisorProps>;
  render?: (args: VisorProps) => JSX.Element;
  name?: string;
};

// ── Individual states ──────────────────────────────────────────────────────────

export const Watching: Story = {
  args: { state: "watching", size: 44 },
  name: "State: Watching (idle blink)",
};

export const Reading: Story = {
  args: { state: "reading", size: 44 },
  name: "State: Reading (scan)",
};

export const Thinking: Story = {
  args: { state: "thinking", size: 44 },
  name: "State: Thinking (pulse)",
};

export const Found: Story = {
  args: { state: "found", size: 44 },
  name: "State: Found (wide eyes)",
};

export const Happy: Story = {
  args: { state: "happy", size: 44 },
  name: "State: Happy (arc eyes)",
};

export const Asleep: Story = {
  args: { state: "asleep", size: 44 },
  name: "State: Asleep (AI paused)",
};

// ── Sizes ─────────────────────────────────────────────────────────────────────

export const Sizes: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: "16px" }}>
      {([14, 18, 24, 32, 44, 64] as const).map((s) => (
        <Visor key={s} size={s} label={`Visor ${s}px`} />
      ))}
    </div>
  ),
  name: "Sizes: 14 → 64px",
};

// ── Accessible label ──────────────────────────────────────────────────────────

export const WithLabel: Story = {
  args: { state: "thinking", size: 44, label: "Scout is thinking" },
  name: "Accessible label",
};

// ── Gaze (pointer-follow) ─────────────────────────────────────────────────────

export const Gaze: Story = {
  args: {
    state: "watching",
    size: 56,
    gaze: true,
    label: "Scout watching you",
  },
  name: "Gaze: pointer-follow",
};
