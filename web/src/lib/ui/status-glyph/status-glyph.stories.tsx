/**
 * StatusGlyph stories — CSF3 format (Storybook 10).
 * Shows all statuses × all places, plus a size range.
 */
import { type JSX } from "react";
import { StatusGlyph, type GlyphStatus } from "./status-glyph";
import { type Place } from "~/lib/ui/signal/signal";

type Meta = {
  title: string;
  tags?: string[];
  parameters?: Record<string, unknown>;
};
type Story = { render: () => JSX.Element; name: string };

const meta: Meta = {
  title: "ui/StatusGlyph",
  tags: ["autodocs"],
  parameters: { layout: "centered" },
};

export default meta;

const ALL_STATUSES: GlyphStatus[] = ["open", "half", "done"];
const ALL_PLACES: Place[] = ["now", "people", "paper", "money", "time", "none"];

const STATUS_LABELS: Record<GlyphStatus, string> = {
  open: "Open",
  half: "In progress",
  done: "Done",
};

// ── All statuses × all places ─────────────────────────────────────────────────

export const Matrix: Story = {
  render: () => (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(6, auto)", gap: "12px", alignItems: "center", justifyItems: "center" }}>
      {ALL_PLACES.map((place) =>
        ALL_STATUSES.map((status) => (
          <StatusGlyph
            key={`${place}-${status}`}
            status={status}
            place={place}
            size={20}
            label={`${STATUS_LABELS[status]} — ${place}`}
          />
        )),
      )}
    </div>
  ),
  name: "Matrix: all statuses × all places",
};

// ── Sizes ─────────────────────────────────────────────────────────────────────

export const Sizes: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
      {([12, 16, 20, 24, 32] as const).map((s) => (
        <StatusGlyph
          key={s}
          status="done"
          place="people"
          size={s}
          label={`Done ${s}px`}
        />
      ))}
    </div>
  ),
  name: "Sizes: 12 → 32px",
};

// ── Individual states ─────────────────────────────────────────────────────────

export const Open: Story = {
  render: () => (
    <StatusGlyph status="open" place="now" size={24} label="Open" />
  ),
  name: "State: Open",
};

export const Half: Story = {
  render: () => (
    <StatusGlyph status="half" place="people" size={24} label="In progress" />
  ),
  name: "State: Half",
};

export const Done: Story = {
  render: () => (
    <StatusGlyph status="done" place="money" size={24} label="Done" />
  ),
  name: "State: Done",
};
