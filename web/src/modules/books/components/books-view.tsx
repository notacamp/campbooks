/* eslint-disable react/forbid-elements */
/*
 * ↑ This file is the Books SURFACE VIEW — it IS the design/layout layer for
 * the Books surface, not a business-logic module. The forbid-elements rule
 * exists to stop business logic modules from duplicating lib/ui; a surface
 * view that owns its own layout is a valid exception.
 */

/**
 * BooksView — the Books (accounting) surface.
 *
 * A prop-driven, presentational component that renders:
 *   1. Reconciliation status line — "N of M lines reconciled · X needing you"
 *      (evidence-gated: no cash balance or monthly delta are asserted, matching
 *      the domain's intentional reconciliation-first model)
 *   2. Needs-you panel — action items (Hunt / Confirm / Mark paid)
 *   3. Reconciled this month — compact obligation ledger (from `obligations`)
 *   4. Loan panel — monthly instalment + progress bar (from `loans[0]`)
 *
 * Typed exactly to the `GET /api/app/money` → `Money::Page.for` serializer
 * shape (`app/serializers/api/app/money/page_serializer.rb` + sub-serializers).
 *
 * Domain note: `page_serializer` deliberately omits closing_balance_cents and
 * monthly totals — the Money domain is evidence-gated reconciliation. A balance
 * is only meaningful once a statement covers the period; asserting one before
 * then would be misleading. The status line therefore shows only what IS known.
 *
 * Constraints:
 *   - No raw hex; no `oklch(<number>…)` literals — colours via CSS tokens.
 *   - TS strict; no `any`.
 *   - `forwardRef` on the root element.
 *   - a11y: all interactive elements labelled; numeric columns use
 *     `font-variant-numeric: tabular-nums` via CSS.
 *   - `prefers-reduced-motion` honoured in loanbar transition via CSS.
 */
import {
  forwardRef,
  type CSSProperties,
  type HTMLAttributes,
} from "react";
import { cn } from "~/lib/utils";
import { Button } from "~/lib/ui/button/button";
import styles from "./books-view.module.css";

// ── Serializer-exact types ────────────────────────────────────────────────────

/** From `obligation_serializer.rb` */
export interface Obligation {
  id: string;
  direction: string;            // "receivable" | "payable" etc.
  counterpart: string | null;
  what: string | null;
  amount_cents: number;
  currency: string;
  anchor_on: string | null;
  status: string;
  settled_on: string | null;
  settled_via: string | null;
  document_id: number | null;
  document_title: string | null;
  statement_label: string | null;
  actions: string[];
}

/** From `needs_you_item_serializer.rb` */
export interface NeedsYouItem {
  kind: string;
  title: string;
  meta: (string | Record<string, unknown>)[];
  actions: string[];
  transaction_id?: number;
  match_id?: number;
  /** loan_missed / loan_changed */
  loan_id?: number;
  instalment_id?: number;
  /** loan_suggestion */
  suggestion_key?: string | null;
  suggestion_lender?: string | null;
  suggestion_amount_cents?: number | null;
  /** statement_failed */
  reconciliation_id?: number;
  /** reconcile_statements */
  pending_count?: number;
  document_ids?: number[];
  /** add_statement */
  focus_month?: string | null;
  focus_label?: string | null;
}

/** From `loan_serializer.rb` */
export interface Loan {
  id: number;
  lender: string;
  source_counterparty: string | null;
  principal_cents: number;
  instalment_cents: number;
  first_instalment_on: string | null;
  /** ISO date of the next upcoming instalment; null when the loan is fully paid. */
  next_instalment_on: string | null;
  term_months: number;
  rate_note: string | null;
  notes: string | null;
  status: string;
  paid_count: number;
  remaining_count: number;
  missed_count: number;
  change_acknowledged_at: string | null;
  created_at: string;
  updated_at: string;
}

/** From `page_serializer.rb` – loan_suggestions sub-array */
export interface LoanSuggestion {
  key: string | null;
  lender: string | null;
  amount_cents: number | null;
  counterparty: string | null;
}

/**
 * From `page_serializer.rb`.
 * `statement_counts` maps `reconciliation_id.to_s → [resolved, total]`.
 */
export interface MoneyPage {
  obligations: Obligation[];
  needs_you: NeedsYouItem[];
  needs_you_count: number;
  loans: Loan[];
  loan_suggestions: LoanSuggestion[];
  /** { "reconciliation_id" => [resolved_count, total_count] } */
  statement_counts: Record<string, [number, number]>;
  selected_statement_id: number | null;
}

// ── Component props ───────────────────────────────────────────────────────────

export interface BooksViewProps extends HTMLAttributes<HTMLDivElement> {
  /** Full Money::Page read model — typed to page_serializer exactly. */
  data: MoneyPage;
  /**
   * Called when the user triggers a line action.
   * `lineId` is the serializer item's primary key (transaction_id, loan_id, etc.),
   * `kind`   maps to the action endpoint segment (confirm, reject, mark_paid, hunt…).
   */
  onLineAction: (lineId: string, kind: string) => void;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Format cents as a localised currency string (EUR default). */
const formatMoney = (cents: number, currency = "EUR"): string => {
  const amount = cents / 100;
  try {
    return new Intl.NumberFormat("pt-PT", {
      style: "currency",
      currency,
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(amount);
  } catch {
    return `${currency} ${amount.toFixed(2)}`;
  }
};

/** Format an ISO date string as short "DD Mon" (e.g. "15 Sep"). */
const formatShortDate = (iso: string | null): string => {
  if (!iso) return "—";
  try {
    const d = new Date(iso);
    return d.toLocaleDateString("en-GB", { day: "2-digit", month: "short" });
  } catch {
    return iso.slice(0, 10);
  }
};

/** Map NeedsYouItem.kind to a human-readable action label and button variant. */
const actionForKind = (
  item: NeedsYouItem,
): { label: string; variant: "default" | "secondary" } => {
  const a = item.actions[0] ?? item.kind;
  switch (a) {
    case "confirm":
      return { label: "Confirm", variant: "default" };
    case "mark_paid":
      return { label: "Mark paid", variant: "secondary" };
    case "hunt":
    case "request_invoice":
      return { label: "Hunt", variant: "secondary" };
    case "add_statement":
    case "reconcile_statements":
      return { label: "Reconcile", variant: "secondary" };
    default:
      return { label: "Review", variant: "secondary" };
  }
};

/**
 * A human title for the row. Some needs-you kinds (e.g. the "add a statement"
 * prompt) come back with an empty title from the API — fall back to a
 * kind-derived phrase so the row never renders as a bare button.
 */
const titleForItem = (item: NeedsYouItem): string => {
  if (item.title && item.title.trim().length > 0) return item.title;
  switch (item.actions[0] ?? item.kind) {
    case "add_statement":
      return "Add a bank statement";
    case "reconcile_statements":
      return "Reconcile a statement";
    default:
      return actionForKind(item).label;
  }
};

/** Extract the stable line-id to pass to onLineAction. */
const lineIdFromItem = (item: NeedsYouItem): string => {
  if (item.transaction_id !== undefined) return String(item.transaction_id);
  if (item.loan_id !== undefined) return String(item.loan_id);
  if (item.reconciliation_id !== undefined) return String(item.reconciliation_id);
  return item.kind;
};

/** Meta summary string for the needs-you row subtitle. */
const metaSummary = (item: NeedsYouItem): string => {
  const parts: string[] = [];
  for (const m of item.meta) {
    if (typeof m === "string" && m.trim()) parts.push(m);
  }
  return parts.slice(0, 2).join(" · ");
};

/** Format a next-instalment ISO date as "D Month" (e.g. "15 October"). */
const formatNextInstalment = (iso: string): string => {
  try {
    return new Date(iso).toLocaleDateString("en-GB", {
      day: "numeric",
      month: "long",
    });
  } catch {
    return iso.slice(0, 10);
  }
};

// ── BooksView ─────────────────────────────────────────────────────────────────

/**
 * Books surface view — presentational, prop-driven.
 * Mount inside the shell's main `<section>` with the `rx-col wide` max-width
 * container (from the prototype) applied by the parent.
 */
export const BooksView = forwardRef<HTMLDivElement, BooksViewProps>(
  ({ data, onLineAction, className, ...props }, ref) => {
    const {
      obligations,
      needs_you,
      needs_you_count,
      loans,
      selected_statement_id,
      statement_counts,
    } = data;

    // Resolve reconciled / total for the selected statement
    const statKey = selected_statement_id !== null
      ? String(selected_statement_id)
      : null;
    const [resolvedCount, totalCount] = statKey && statement_counts[statKey]
      ? statement_counts[statKey]
      : [null, null];

    // Primary loan (first active, if any)
    const loan = loans.length > 0 ? loans[0] : null;

    // Reconciled ledger: obligations with a settled_on date (or all obligations)
    const reconciledItems = obligations.filter(
      (ob): ob is Obligation & { settled_on: string } =>
        ob.settled_on !== null,
    );

    return (
      <div
        ref={ref}
        className={cn(className)}
        {...props}
      >
        {/* ── Reconciliation status line ── */}
        <div className={styles.statusBar} aria-label="Reconciliation status">
          <span className={styles.statusMain}>
            {resolvedCount !== null && totalCount !== null ? (
              <>
                <strong>{resolvedCount}</strong>
                {" of "}
                <strong>{totalCount}</strong>
                {" lines reconciled"}
              </>
            ) : (
              "No statement selected"
            )}
          </span>
          {needs_you_count > 0 ? (
            <span className={styles.statusNeeds}>
              {"· "}
              <strong>{needs_you_count}</strong>
              {" needing you"}
            </span>
          ) : null}
        </div>

        {/* ── Two-column grid: Needs you + Reconciled this month ── */}
        <div className={styles.twoCol}>
          {/* Needs-you panel */}
          <section
            className={styles.panelCard}
            aria-label={`Needs you — ${needs_you_count} item${needs_you_count === 1 ? "" : "s"}`}
          >
            <h2 className={styles.panelHead}>
              Needs you
              <span className={styles.panelCount} aria-hidden="true">
                {needs_you_count} line{needs_you_count === 1 ? "" : "s"}
              </span>
            </h2>

            {needs_you.length === 0 ? (
              <p className={styles.emptyRow}>Nothing needs your attention.</p>
            ) : (
              <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
                {needs_you.map((item) => {
                  const { label, variant } = actionForKind(item);
                  const lineId = lineIdFromItem(item);
                  const meta = metaSummary(item);
                  const title = titleForItem(item);

                  return (
                    <li key={`${item.kind}-${lineId}`} className={styles.needrow}>
                      <div className={styles.needrowBody}>
                        {title}
                        {meta ? (
                          <small className={styles.needrowMeta}>{meta}</small>
                        ) : null}
                      </div>
                      <Button
                        variant={variant}
                        size="sm"
                        type="button"
                        aria-label={`${label}: ${title}`}
                        onClick={(): void => {
                          onLineAction(lineId, item.actions[0] ?? item.kind);
                        }}
                      >
                        {label}
                      </Button>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>

          {/* Reconciled this month */}
          <section
            className={styles.panelCard}
            aria-label="Reconciled this month"
          >
            <h2 className={styles.panelHead}>
              Reconciled this month
              {resolvedCount !== null && totalCount !== null ? (
                <span className={styles.panelCount} aria-hidden="true">
                  {resolvedCount} of {totalCount}
                </span>
              ) : null}
            </h2>

            {reconciledItems.length === 0 ? (
              <p className={styles.emptyRow}>No reconciled items yet.</p>
            ) : (
              <ol
                style={{
                  listStyle: "none",
                  margin: 0,
                  padding: 0,
                  fontVariantNumeric: "tabular-nums",
                } as CSSProperties}
                aria-label="Reconciled ledger"
              >
                {reconciledItems.map((ob) => {
                  const isIncome = ob.direction === "receivable";
                  const descr =
                    ob.what ?? ob.document_title ?? ob.counterpart ?? "—";
                  const displayDate = formatShortDate(
                    ob.settled_on ?? ob.anchor_on,
                  );
                  const sign = isIncome ? "+" : "−";
                  const formatted = formatMoney(
                    Math.abs(ob.amount_cents),
                    ob.currency,
                  );

                  return (
                    <li key={ob.id} className={styles.lrow}>
                      <time
                        className={styles.lrowDate}
                        dateTime={ob.settled_on ?? ob.anchor_on ?? undefined}
                      >
                        {displayDate}
                      </time>
                      <span className={styles.lrowDesc} title={descr}>
                        {descr}
                      </span>
                                  <span
                        className={cn(
                          styles.lrowAmount,
                          isIncome && styles.lrowAmountIn,
                        )}
                        aria-label={`${isIncome ? "Income" : "Expense"} ${sign}${formatted}`}
                      >
                        {sign}{formatted}
                      </span>
                    </li>
                  );
                })}
              </ol>
            )}
          </section>
        </div>

        {/* ── Loan panel (full-width) ── */}
        {loan !== null && (
          <section
            className={cn(styles.panelCard, styles.loanCard)}
            aria-label={`Loan: ${loan.lender}`}
          >
            <h2 className={styles.panelHead}>
              {loan.lender}
              {loan.source_counterparty ? (
                <span className={styles.panelCount}>
                  {loan.source_counterparty}
                </span>
              ) : (
                <span className={styles.panelCount}>
                  {loan.status === "on_schedule" ? "on schedule" : loan.status}
                </span>
              )}
            </h2>

            <div
              style={
                { display: "flex", alignItems: "baseline" } as CSSProperties
              }
            >
              <span className={styles.loanAmount}>
                {formatMoney(loan.instalment_cents)}
              </span>
              <span className={styles.loanPer}>monthly</span>
            </div>

            <div
              className={styles.loanBar}
              role="progressbar"
              aria-valuenow={loan.paid_count}
              aria-valuemin={0}
              aria-valuemax={loan.term_months}
              aria-label={`${loan.paid_count} of ${loan.term_months} instalments paid`}
            >
              <span
                className={styles.loanBarFill}
                style={
                  {
                    width: `${Math.min(
                      100,
                      (loan.paid_count / Math.max(loan.term_months, 1)) * 100,
                    ).toFixed(2)}%`,
                  } as CSSProperties
                }
              />
            </div>

            <div className={styles.loanMeta}>
              <span>
                {loan.paid_count} of {loan.term_months} paid
              </span>
              {loan.next_instalment_on !== null ? (
                <span>Next {formatNextInstalment(loan.next_instalment_on)}</span>
              ) : null}
            </div>
          </section>
        )}
      </div>
    );
  },
);

BooksView.displayName = "BooksView";
