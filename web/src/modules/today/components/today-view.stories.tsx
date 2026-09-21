/**
 * TodayView stories — CSF3 format (Storybook 10).
 *
 * Three stories:
 *   1. FullDeck   — reply / pay / decide; one with draft:true; one overdue
 *   2. SingleItem — one item only
 *   3. AllClear   — empty needs_you (cleared state)
 *
 * Uses locally-typed meta so the file compiles without @storybook/react in
 * package.json (mirrors the Skim pattern).
 */

/* eslint-disable react/forbid-elements -- stories render layout scaffolding */
/* eslint-disable react/forbid-component-props -- stories pass style for container sizing */

import { useState, type JSX } from "react";
import { TodayView, type TodayData, type TodayViewProps } from "./today-view";

// ── CSF3 meta ─────────────────────────────────────────────────────────────────

const meta = {
  title: "modules/today/TodayView",
  component: TodayView,
  tags: ["autodocs"],
  parameters: { layout: "padded" },
};

export default meta;

type Story = {
  name?: string;
  render?: () => JSX.Element;
  args?: Partial<TodayViewProps>;
};

// ── Shared fixture data ────────────────────────────────────────────────────────

const HANDLED_BASE: TodayData["handled"] = {
  filed: 41,
  matched: 2,
  tucked: 4,
  added: 1,
  since: "2026-09-21",
};

const COMING_UP_BASE: TodayData["coming_up"] = [
  {
    on: "2026-09-24",
    label: "Review for Sofia",
    sub: "held 09:00 · backup slot",
    place: "Time",
  },
  {
    on: "2026-09-25",
    label: "Cloudhost invoice due",
    sub: "€148.00",
    place: "Money",
  },
  {
    on: "2026-10-30",
    label: "Studio domain renews",
    sub: "auto-charge €18",
    place: "Now",
  },
];

// ── Interactive wrapper ────────────────────────────────────────────────────────

const TodayStory = ({ data }: { data: TodayData }): JSX.Element => {
  const [log, setLog] = useState<string[]>([]);

  const handleAction = (itemId: string, actionKey: string): void => {
    setLog((prev) =>
      [`onAction(${itemId}, ${actionKey})`, ...prev].slice(0, 6),
    );
  };

  const handleUndo = (itemId: string): void => {
    setLog((prev) => [`onUndo(${itemId})`, ...prev].slice(0, 6));
  };

  const handleSkim = (): void => {
    setLog((prev) => ["onSkim()", ...prev].slice(0, 6));
  };

  return (
    <div
      style={{
        background: "var(--cb-bg)",
        minHeight: "100vh",
        padding: "32px 40px",
      }}
    >
      <TodayView
        data={data}
        onAction={handleAction}
        onUndo={handleUndo}
        onSkim={handleSkim}
        style={{ maxWidth: "720px", margin: "0 auto" }}
      />
      {log.length > 0 && (
        <div
          style={{
            marginTop: "32px",
            padding: "12px 16px",
            borderRadius: "var(--cb-r2)",
            background: "var(--cb-s1)",
            boxShadow: "inset 0 0 0 1px var(--cb-line)",
            fontFamily: "monospace",
            fontSize: "12px",
            color: "var(--cb-t3)",
          }}
        >
          {log.map((entry, i) => (
            <div key={i}>{entry}</div>
          ))}
        </div>
      )}
    </div>
  );
};

// ── Stories ───────────────────────────────────────────────────────────────────

export const FullDeck: Story = {
  name: "Full deck (reply / pay / decide)",
  render: (): JSX.Element => (
    <TodayStory
      data={{
        greeting: {
          name: "Ana",
          date: "2026-09-21",
          brief:
            "I read 48 emails since Friday evening. 41 are filed and 4 newsletters are tucked away. Three things need you today, and you have two deadlines this week.",
        },
        needs_you: [
          {
            id: "people_101",
            source: "people",
            ref_id: 101,
            verb: "reply",
            place: "Inbox",
            title: "Reply to Sofia Pessoa",
            subtitle: "Brightloop · Q4 proposal",
            read: "She asked you to review the pricing before Thursday — she sends it to her board on Friday.",
            due: "2026-09-25",
            draft: true,
            overdue: false,
            actions: [
              { kind: "reply", label: "Review draft", primary: true },
              { kind: "snooze", label: "Hold time", primary: false },
              { kind: "dismiss", label: "Not now", primary: false },
            ],
          },
          {
            id: "money_0_invoice",
            source: "money",
            ref_id: "ch-20418",
            verb: "pay",
            place: "Money",
            title: "Pay Cloudhost",
            subtitle: "Invoice CH-20418 · €148.00",
            read: "Due Friday. It's not on a statement yet, so I can't tell if it's paid.",
            due: "2026-09-26",
            draft: null,
            overdue: false,
            actions: [
              { kind: "paid", label: "I've paid it", primary: true },
              { kind: "snooze", label: "Remind Thursday", primary: false },
              { kind: "dismiss", label: "Not now", primary: false },
            ],
          },
          {
            id: "time_task_55",
            source: "time",
            ref_id: 55,
            verb: "decide",
            place: "Time",
            title: "Decide a date for Marta",
            subtitle: "Office move",
            read: "She needs a date. You're free Tuesday 14:00–16:00 — I can hold it and draft the confirmation.",
            due: "2026-09-23",
            draft: null,
            overdue: true,
            actions: [
              { kind: "confirm", label: "Hold Tue 14:00", primary: true },
              { kind: "snooze", label: "Pick another", primary: false },
              { kind: "dismiss", label: "Not now", primary: false },
            ],
          },
        ],
        coming_up: COMING_UP_BASE,
        handled: HANDLED_BASE,
      }}
    />
  ),
};

export const SingleItem: Story = {
  name: "Single item",
  render: (): JSX.Element => (
    <TodayStory
      data={{
        greeting: {
          name: "Ana",
          date: "2026-09-21",
          brief: "1 thing needs you.",
        },
        needs_you: [
          {
            id: "people_202",
            source: "people",
            ref_id: 202,
            verb: "chase",
            place: "Inbox",
            title: "Chase Rui Nogueira",
            subtitle: "Logo files for the printer",
            read: "You asked him last Tuesday. No reply yet — follow up?",
            due: null,
            draft: null,
            overdue: false,
            actions: [
              { kind: "reply", label: "Follow up", primary: true },
              { kind: "snooze", label: "Later", primary: false },
            ],
          },
        ],
        coming_up: [
          {
            on: "2026-09-22",
            label: "Team standup",
            sub: "09:30 · Google Meet",
            place: "Calendar",
          },
        ],
        handled: {
          filed: 12,
          matched: 0,
          tucked: 1,
          added: 0,
          since: "2026-09-21",
        },
      }}
    />
  ),
};

export const AllClear: Story = {
  name: "All clear (empty needs_you)",
  render: (): JSX.Element => (
    <TodayStory
      data={{
        greeting: {
          name: "Ana",
          date: "2026-09-21",
          brief: "Nothing urgent today.",
        },
        needs_you: [],
        coming_up: COMING_UP_BASE,
        handled: HANDLED_BASE,
      }}
    />
  ),
};
