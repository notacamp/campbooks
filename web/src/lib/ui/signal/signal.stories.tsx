/**
 * Signal stories — CSF3 format (Storybook 10).
 * Covers Signal.Dot, Signal.Label, and Signal.Pill across all places.
 */
import { type JSX } from "react";
import { Signal, type Place } from "./signal";

// Minimal local CSF3 types
type Meta = {
  title: string;
  tags?: string[];
  parameters?: Record<string, unknown>;
};
type Story = { render: () => JSX.Element; name: string };

const meta: Meta = {
  title: "ui/Signal",
  tags: ["autodocs"],
  parameters: { layout: "centered" },
};

export default meta;

const ALL_PLACES: Place[] = ["now", "people", "paper", "money", "time", "none"];

const PLACE_LABELS: Record<Place, string> = {
  now: "Now",
  people: "People",
  paper: "Paper",
  money: "Money",
  time: "Time",
  none: "None",
};

// ── Dots ─────────────────────────────────────────────────────────────────────

export const Dots: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
      {ALL_PLACES.map((place) => (
        <Signal.Dot key={place} place={place} aria-label={`${PLACE_LABELS[place]} signal`} />
      ))}
    </div>
  ),
  name: "Dot: all places",
};

// ── Labels ────────────────────────────────────────────────────────────────────

export const Labels: Story = {
  render: () => (
    <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
      {ALL_PLACES.map((place) => (
        <Signal.Label key={place} place={place}>
          {PLACE_LABELS[place]}
        </Signal.Label>
      ))}
    </div>
  ),
  name: "Label: all places",
};

// ── Pills ─────────────────────────────────────────────────────────────────────

export const Pills: Story = {
  render: () => (
    <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
      {ALL_PLACES.map((place) => (
        <Signal.Pill key={place} place={place}>
          {PLACE_LABELS[place]}
        </Signal.Pill>
      ))}
    </div>
  ),
  name: "Pill: all places",
};

// ── All together ──────────────────────────────────────────────────────────────

export const AllVariants: Story = {
  render: () => (
    <div style={{ display: "grid", gap: "16px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <span style={{ width: "48px", fontSize: "11px", color: "var(--cb-t3)" }}>Dot</span>
        <div style={{ display: "flex", gap: "12px" }}>
          {ALL_PLACES.map((place) => (
            <Signal.Dot key={place} place={place} aria-label={PLACE_LABELS[place]} />
          ))}
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <span style={{ width: "48px", fontSize: "11px", color: "var(--cb-t3)" }}>Label</span>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
          {ALL_PLACES.map((place) => (
            <Signal.Label key={place} place={place}>{PLACE_LABELS[place]}</Signal.Label>
          ))}
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <span style={{ width: "48px", fontSize: "11px", color: "var(--cb-t3)" }}>Pill</span>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
          {ALL_PLACES.map((place) => (
            <Signal.Pill key={place} place={place}>{PLACE_LABELS[place]}</Signal.Pill>
          ))}
        </div>
      </div>
    </div>
  ),
  name: "All variants × all places",
};
