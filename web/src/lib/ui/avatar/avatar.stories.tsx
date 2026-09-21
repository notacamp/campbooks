/**
 * Avatar stories — CSF3 format (Storybook 10).
 * Shows person vs service, all places, derived initials, and a size range.
 */
import { type JSX } from "react";
import { Avatar } from "./avatar";
import { type Place } from "~/lib/ui/signal/signal";

type Meta = {
  title: string;
  tags?: string[];
  parameters?: Record<string, unknown>;
};
type Story = { render: () => JSX.Element; name: string };

const meta: Meta = {
  title: "ui/Avatar",
  tags: ["autodocs"],
  parameters: { layout: "centered" },
};

export default meta;

const ALL_PLACES: Place[] = ["now", "people", "paper", "money", "time", "none"];

// ── Person avatars ────────────────────────────────────────────────────────────

export const PersonAllPlaces: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
      {ALL_PLACES.map((place) => (
        <Avatar key={place} initials="GU" place={place} kind="person" aria-label={`Person avatar — ${place}`} />
      ))}
    </div>
  ),
  name: "Person: all places",
};

// ── Service avatars ───────────────────────────────────────────────────────────

export const ServiceAllPlaces: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
      {ALL_PLACES.map((place) => (
        <Avatar key={place} initials="SV" place={place} kind="service" aria-label={`Service avatar — ${place}`} />
      ))}
    </div>
  ),
  name: "Service: all places",
};

// ── Derived from email ────────────────────────────────────────────────────────

export const DerivedFromEmail: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
      {["alice@example.com", "bob.smith@corp.io", "support@shop.co", "n@x.com"].map(
        (email) => (
          <Avatar key={email} email={email} place="people" kind="person" />
        ),
      )}
    </div>
  ),
  name: "Initials derived from email",
};

// ── Sizes ─────────────────────────────────────────────────────────────────────

export const Sizes: Story = {
  render: () => (
    <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
      {([20, 24, 30, 36, 44, 56] as const).map((s) => (
        <Avatar key={s} initials="GU" place="people" kind="person" size={s} aria-label={`Avatar ${s}px`} />
      ))}
    </div>
  ),
  name: "Sizes: 20 → 56px",
};

// ── Stack (offset overlap) ────────────────────────────────────────────────────

export const Stack: Story = {
  render: () => (
    <div style={{ display: "flex" }}>
      {(["now", "people", "paper"] as Place[]).map((place, i) => (
        <span
          key={place}
          style={{ marginLeft: i === 0 ? 0 : "-8px", boxShadow: "0 0 0 2px var(--cb-s1)", borderRadius: "50%", display: "inline-block" }}
        >
          <Avatar initials="GU" place={place} kind="person" size={30} />
        </span>
      ))}
    </div>
  ),
  name: "Stacked overlap",
};
