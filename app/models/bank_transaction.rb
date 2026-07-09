# frozen_string_literal: true

# One line from an imported bank statement. `amount_cents` is SIGNED:
# negative = money out (debit), positive = money in (credit).
# Ordered by `position` (the original CSV row index) for stable display.
class BankTransaction < ApplicationRecord
  belongs_to :reconciliation
  belongs_to :workspace
  belongs_to :requested_by, class_name: "User", optional: true

  has_many :transaction_matches, dependent: :destroy
  has_many :matched_documents, through: :transaction_matches, source: :document

  # Matching lifecycle for this individual transaction.
  enum :status, {
    unmatched: 0,
    suggested: 1,
    matched:   2,
    excluded:  3,
    requested: 4  # invoice requested from counterparty
  }

  validates :position, uniqueness: { scope: :reconciliation_id }
  validates :booked_on, presence: true
  validates :description, presence: true
  validates :amount_cents, presence: true

  scope :ordered, -> { order(position: :asc) }

  # ── Amount helpers ──────────────────────────────────────────────────────────

  def debit?
    amount_cents.negative?
  end

  def credit?
    amount_cents >= 0
  end

  # Which Document types to suggest when hunting for a match. Debits (money out)
  # should pair with expenses; credits (money in) with revenue documents.
  def candidate_document_types
    if debit?
      %w[expense_invoice receipt credit_note]
    else
      %w[revenue_invoice]
    end
  end
end
