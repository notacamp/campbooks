/**
 * CalendarView stories — CSF3 format.
 *
 * Fixture data uses CSS variable references (e.g. "var(--cb-people)") for
 * event colours so no raw hex literals appear in source. In production the
 * API returns real hex strings from the calendar provider.
 *
 * Stories:
 *   1. ThisWeek  — Today (standup, call, focus block, deadline) + Thu + Fri
 *   2. EmptyWeek — no items
 *   3. TodayOnly — a single day with all kinds + a day note
 */

/* eslint-disable react/forbid-elements */

import { type JSX } from "react";
import { CalendarView, type TimeAgenda } from "./calendar-view";

// ── CSF3 meta ────────────────────────────────────────────────────────────────

const meta = {
  title: "modules/Calendar/CalendarView",
  component: CalendarView,
  tags: ["autodocs"],
  parameters: { layout: "padded", backgrounds: { default: "dark" } },
};

export default meta;

type Story = {
  name?: string;
  render?: () => JSX.Element;
  args?: Partial<{ data: TimeAgenda }>;
};

// ── Fixture data ─────────────────────────────────────────────────────────────

/** Base date anchored to a Monday for readability. */
const TODAY = "2026-09-21";
const THU = "2026-09-24";
const FRI = "2026-09-26";

const THIS_WEEK_DATA: TimeAgenda = {
  items: [
    // ── Today ──────────────────────────────────────────────────────────────
    {
      kind: "event",
      id: 101,
      title: "Studio standup",
      at: `${TODAY}T09:30:00`,
      day: TODAY,
      all_day: false,
      overdue: false,
      duration_minutes: 15,
      // CSS var reference — in prod this would be a hex from Google Calendar
      color: "var(--cb-people)",
      source_label: null,
      source_path: null,
      emphasis: null,
      why: null,
      prep_name: null,
      prep_detail: null,
      handed: null,
      actions: null,
      event: {
        id: 101,
        provider_event_id: "abc123",
        join_url: null,
        rsvp_status: "accepted",
        calendar_id: 1,
      },
    },
    {
      kind: "event",
      id: 102,
      title: "Call with Brightloop",
      at: `${TODAY}T11:00:00`,
      day: TODAY,
      all_day: false,
      overdue: false,
      duration_minutes: 60,
      color: "var(--cb-people)",
      source_label: "Sofia, Rui · from her email",
      source_path: null,
      emphasis: null,
      why: null,
      prep_name: "Sofia",
      prep_detail: "Review the Q4 proposal",
      handed: null,
      actions: null,
      event: {
        id: 102,
        provider_event_id: "def456",
        join_url: "https://meet.example.com/brightloop",
        rsvp_status: "accepted",
        calendar_id: 1,
      },
    },
    {
      kind: "focus",
      id: 201,
      title: "Focus: Q4 review",
      at: `${TODAY}T14:00:00`,
      day: TODAY,
      all_day: false,
      overdue: false,
      duration_minutes: 120,
      color: null,
      source_label: "Scout held two free hours",
      source_path: null,
      emphasis: null,
      why: null,
      prep_name: null,
      prep_detail: null,
      handed: null,
      actions: null,
      focus_block: {
        id: 201,
        status: "kept",
        task_id: 55,
        calendar_event_id: null,
      },
    },
    {
      kind: "deadline",
      id: 301,
      title: "VAT documents due",
      at: `${TODAY}T18:00:00`,
      day: TODAY,
      all_day: false,
      overdue: false,
      duration_minutes: null,
      color: null,
      source_label: null,
      source_path: null,
      emphasis: true,
      why: "3 to review",
      prep_name: null,
      prep_detail: null,
      handed: null,
      actions: null,
      reminder: {
        id: 301,
        status: "pending",
        reminder_type: "document_deadline",
        snoozed_until: null,
      },
    },

    // ── Thursday ───────────────────────────────────────────────────────────
    {
      kind: "event",
      id: 103,
      title: "Held: review for Sofia",
      at: `${THU}T09:00:00`,
      day: THU,
      all_day: false,
      overdue: false,
      duration_minutes: 60,
      color: "var(--cb-time)",
      source_label: null,
      source_path: null,
      emphasis: null,
      why: null,
      prep_name: null,
      prep_detail: null,
      handed: null,
      actions: null,
      event: {
        id: 103,
        provider_event_id: "ghi789",
        join_url: null,
        rsvp_status: "accepted",
        calendar_id: 1,
      },
    },

    // ── Friday ─────────────────────────────────────────────────────────────
    {
      kind: "deadline",
      id: 302,
      title: "Cloudhost invoice due",
      at: null,
      day: FRI,
      all_day: true,
      overdue: false,
      duration_minutes: null,
      color: null,
      source_label: null,
      source_path: null,
      emphasis: null,
      why: "€148.00",
      prep_name: null,
      prep_detail: null,
      handed: null,
      actions: null,
      reminder: {
        id: 302,
        status: "pending",
        reminder_type: "invoice_due",
        snoozed_until: null,
      },
    },
  ],

  undated: [],

  day_note: {
    date: TODAY,
    meetings_count: 2,
    deadlines_count: 1,
    first_deadline_title: "VAT documents",
    undated_count: 0,
    focus: { title: "Q4 review", subject: null, at: `${TODAY}T14:00:00`, duration_minutes: 120 },
    late_obligation: null,
    prep_meeting: null,
  },

  suggestions: [
    { starts_at: `${TODAY}T16:00:00`, ends_at: `${TODAY}T17:00:00`, duration_minutes: 60 },
  ],
};

const EMPTY_WEEK_DATA: TimeAgenda = {
  items: [],
  undated: [],
  day_note: null,
  suggestions: [],
};

const TODAY_ONLY_DATA: TimeAgenda = {
  items: [
    {
      kind: "event",
      id: 200,
      title: "Weekly review",
      at: `${TODAY}T10:00:00`,
      day: TODAY,
      all_day: false,
      overdue: false,
      duration_minutes: 30,
      color: "var(--cb-people)",
      source_label: null,
      source_path: null,
      emphasis: null,
      why: null,
      prep_name: null,
      prep_detail: null,
      handed: null,
      actions: null,
      event: {
        id: 200,
        provider_event_id: "jkl",
        join_url: null,
        rsvp_status: "accepted",
        calendar_id: 1,
      },
    },
    {
      kind: "deadline",
      id: 400,
      title: "Insurance renewal",
      at: `${TODAY}T23:59:00`,
      day: TODAY,
      all_day: false,
      overdue: true,
      duration_minutes: null,
      color: null,
      source_label: null,
      source_path: null,
      emphasis: true,
      why: "3 days overdue",
      prep_name: null,
      prep_detail: null,
      handed: null,
      actions: null,
      reminder: {
        id: 400,
        status: "pending",
        reminder_type: "document_deadline",
        snoozed_until: null,
      },
    },
    {
      kind: "focus",
      id: 300,
      title: "Focus: client proposal",
      at: `${TODAY}T15:00:00`,
      day: TODAY,
      all_day: false,
      overdue: false,
      duration_minutes: 90,
      color: null,
      source_label: "Scout proposed this slot",
      source_path: null,
      emphasis: null,
      why: null,
      prep_name: null,
      prep_detail: null,
      handed: null,
      actions: null,
      focus_block: {
        id: 300,
        status: "proposed",
        task_id: 88,
        calendar_event_id: null,
      },
    },
  ],
  undated: [],
  day_note: {
    date: TODAY,
    meetings_count: 1,
    deadlines_count: 1,
    first_deadline_title: "Insurance renewal",
    undated_count: 2,
    focus: { title: "client proposal", subject: null, at: `${TODAY}T15:00:00`, duration_minutes: 90 },
    late_obligation: {
      name: "Insurance renewal",
      days_overdue: 3,
    },
    prep_meeting: null,
  },
  suggestions: [],
};

// ── Wrapper ──────────────────────────────────────────────────────────────────

const Wrapper = ({ data }: { data: TimeAgenda }): JSX.Element => (
  <div
    style={{
      maxWidth: 640,
      margin: "0 auto",
      padding: "20px 16px",
      background: "var(--cb-bg)",
      minHeight: 400,
      fontFamily: "var(--cb-font)",
      color: "var(--cb-t1)",
    }}
  >
    <CalendarView data={data} />
  </div>
);

// ── Stories ──────────────────────────────────────────────────────────────────

export const ThisWeek: Story = {
  name: "This week",
  render: () => <Wrapper data={THIS_WEEK_DATA} />,
};

export const EmptyWeek: Story = {
  name: "Empty week",
  render: () => <Wrapper data={EMPTY_WEEK_DATA} />,
};

export const TodayOnly: Story = {
  name: "Today only — with overdue + late obligation note",
  render: () => <Wrapper data={TODAY_ONLY_DATA} />,
};
