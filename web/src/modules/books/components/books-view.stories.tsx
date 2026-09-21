/* eslint-disable react/forbid-elements */
/* Stories require raw layout divs for the shell wrapper. */

/**
 * BooksView stories — CSF3 format (Storybook 10).
 *
 * Two stories:
 *   1. WithData        — realistic fictional data: 3 needs-you lines, 4 reconciled
 *                        obligations, a loan at 14/60.
 *   2. AllReconciled   — empty needs-you panel, a full reconciled ledger, same loan.
 *
 * Stories use locally-typed meta so the file compiles without
 * @storybook/react in package.json (mirrors the skim.stories.tsx pattern).
 */
import { type JSX } from "react";
import { BooksView, type BooksViewProps, type MoneyPage, type Obligation, type NeedsYouItem, type Loan } from "./books-view";

// ── CSF3 meta ────────────────────────────────────────────────────────────────

const meta = {
  title: "ui/BooksView",
  component: BooksView,
  tags: ["autodocs"],
  parameters: { layout: "padded" },
};

export default meta;

type Story = {
  name?: string;
  render?: () => JSX.Element;
  args?: Partial<BooksViewProps>;
};

// ── Story wrapper ─────────────────────────────────────────────────────────────

const StoryShell = ({ data }: { data: MoneyPage }): JSX.Element => {
  const handleLineAction = (_lineId: string, _kind: string): void => {
    // No-op in stories — callers perform the real mutation.
  };

  return (
    <div
      style={{
        maxWidth: "940px",
        margin: "0 auto",
        padding: "26px 24px",
        background: "var(--cb-bg)",
        minHeight: "100vh",
        fontFamily: "var(--cb-font)",
        color: "var(--cb-t1)",
      }}
    >
      <p
        style={{
          fontSize: "13px",
          fontWeight: 600,
          color: "var(--cb-t3)",
          marginBottom: "4px",
        }}
      >
        September 2026 · Northbank ··0419
      </p>
      <h1
        style={{
          fontFamily: "var(--cb-font-display)",
          fontWeight: 700,
          fontSize: "2.4rem",
          letterSpacing: "-0.035em",
          lineHeight: 1.05,
          margin: "6px 0 16px",
          color: "var(--cb-t1)",
        }}
      >
        Books
      </h1>
      <BooksView data={data} onLineAction={handleLineAction} />
    </div>
  );
};

// ── Fixtures ──────────────────────────────────────────────────────────────────

const LOAN_14_60: Loan = {
  id: 1,
  lender: "Equipment loan · Northbank",
  source_counterparty: "Northbank",
  principal_cents: 3_000_000,
  instalment_cents: 61_237,
  first_instalment_on: "2025-03-15",
  next_instalment_on: "2026-10-15",
  term_months: 60,
  rate_note: "4.9% APR",
  notes: null,
  status: "on_schedule",
  paid_count: 14,
  remaining_count: 46,
  missed_count: 0,
  change_acknowledged_at: null,
  created_at: "2025-03-01T00:00:00Z",
  updated_at: "2026-09-15T00:00:00Z",
};

const NEEDS_YOU_3: NeedsYouItem[] = [
  {
    kind: "hunt",
    title: "PAG SERVICOS 40213",
    meta: ["18 Sep · −€64.90 · no document"],
    actions: ["hunt"],
    transaction_id: 101,
  },
  {
    kind: "confirm",
    title: "CARD FRAMEBOX",
    meta: ["11 Sep · −€29.00 · receipt found in Inbox"],
    actions: ["confirm"],
    transaction_id: 102,
    match_id: 201,
  },
  {
    kind: "mark_paid",
    title: "Cloudhost CH-20418",
    meta: ["due Fri · −€148.00 · unpaid"],
    actions: ["mark_paid"],
    transaction_id: 103,
  },
];

const RECONCILED_OBLIGATIONS: Obligation[] = [
  {
    id: "doc:aaa1",
    direction: "payable",
    counterpart: "Northbank",
    what: "Loan · instalment 14/60",
    amount_cents: -61_237,
    currency: "EUR",
    anchor_on: "2026-09-15",
    status: "settled",
    settled_on: "2026-09-15",
    settled_via: "statement",
    document_id: null,
    document_title: null,
    statement_label: "September",
    actions: [],
  },
  {
    id: "doc:aaa2",
    direction: "receivable",
    counterpart: "Brightloop",
    what: "Invoice 2026-091 · Brightloop",
    amount_cents: 248_050,
    currency: "EUR",
    anchor_on: "2026-09-14",
    status: "settled",
    settled_on: "2026-09-14",
    settled_via: "statement",
    document_id: 301,
    document_title: "Invoice 2026-091",
    statement_label: "September",
    actions: [],
  },
  {
    id: "doc:aaa3",
    direction: "payable",
    counterpart: "Voltio Energia",
    what: "Voltio Energia · August",
    amount_cents: -9_140,
    currency: "EUR",
    anchor_on: "2026-09-08",
    status: "settled",
    settled_on: "2026-09-08",
    settled_via: "statement",
    document_id: 302,
    document_title: "August utility bill",
    statement_label: "September",
    actions: [],
  },
  {
    id: "doc:aaa4",
    direction: "receivable",
    counterpart: "Atelier Nunes",
    what: "Invoice 2026-088 · Atelier Nunes",
    amount_cents: 120_000,
    currency: "EUR",
    anchor_on: "2026-09-03",
    status: "settled",
    settled_on: "2026-09-03",
    settled_via: "statement",
    document_id: 303,
    document_title: "Invoice 2026-088",
    statement_label: "September",
    actions: [],
  },
];

// ── Story: WithData ───────────────────────────────────────────────────────────

const dataWithNeedsYou: MoneyPage = {
  obligations: RECONCILED_OBLIGATIONS,
  needs_you: NEEDS_YOU_3,
  needs_you_count: 3,
  loans: [LOAN_14_60],
  loan_suggestions: [],
  statement_counts: { "9f8e7d6c": [31, 38] },
  selected_statement_id: null,
};

export const WithData: Story = {
  name: "With data — 3 needs-you, 4 reconciled, loan 14/60",
  render: (): JSX.Element => <StoryShell data={dataWithNeedsYou} />,
};

// ── Story: AllReconciled ──────────────────────────────────────────────────────

const dataAllReconciled: MoneyPage = {
  obligations: RECONCILED_OBLIGATIONS,
  needs_you: [],
  needs_you_count: 0,
  loans: [LOAN_14_60],
  loan_suggestions: [],
  statement_counts: { "9f8e7d6c": [38, 38] },
  selected_statement_id: null,
};

export const AllReconciled: Story = {
  name: "All reconciled — empty needs-you",
  render: (): JSX.Element => <StoryShell data={dataAllReconciled} />,
};
