# frozen_string_literal: true

# A bank-statement reconciliation session. The user imports a bank statement
# (as a CSV or PDF Document) and the system parses it into BankTransactions
# that can then be matched against workspace Documents (expense invoices, receipts,
# revenue invoices). Gated behind Features.accounting? and the :accounting
# billing entitlement.
class Reconciliation < ApplicationRecord
  belongs_to :workspace
  belongs_to :created_by, class_name: "User"
  belongs_to :statement_document, class_name: "Document"

  has_many :bank_transactions, dependent: :destroy
  has_one_attached :export_zip

  # Parsing / matching lifecycle. Integer-backed — APPEND new values, never reorder.
  enum :status, {
    pending:  0,
    parsing:  1,
    matching: 2,
    ready:    3,
    failed:   4
  }

  # Export lifecycle — values carry the export_ prefix to avoid collision with `status`.
  # (No Rails prefix: option needed since the value names already include it.)
  enum :export_status, {
    export_none:       0,
    export_generating: 1,
    export_generated:  2,
    export_failed:     3
  }

  scope :recent, -> { order(created_at: :desc) }

  # ── Computed helpers ─────────────────────────────────────────────────────────

  def total_transactions
    bank_transactions.count
  end

  # Transactions that are no longer "unmatched" — matched, excluded, or with a
  # pending invoice request count as "resolved" for the progress counter.
  def resolved_count
    bank_transactions.where(status: %i[matched excluded requested]).count
  end

  def progress_label
    "#{resolved_count}/#{total_transactions}"
  end

  def period_label
    return nil if period_start.blank? && period_end.blank?

    parts = [ period_start, period_end ].compact.map { |d| d.strftime("%-d %b %Y") }
    parts.uniq.join(" – ")
  end
end
