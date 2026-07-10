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

  # ── NIF / amount helpers ────────────────────────────────────────────────────

  # Returns true when the top confirmed match's document has a NIF issue
  # (missing or mismatch) for the given company NIF. Single source of truth
  # used by the controller (request_invoice gate) and the resolve panel prefill.
  def nif_flagged?(company_nif)
    return false if company_nif.blank?

    top = transaction_matches.select(&:confirmed?).max_by(&:confidence)
    return false unless top

    top.document.nif_status(company_nif)&.in?(%i[missing mismatch]) || false
  end

  # Signed amount string used in invoice-request email subjects and bodies.
  # Single source of truth shared by the controller and the resolve panel.
  # e.g. "-45.90 EUR" for a debit, "+1200.00 EUR" for a credit.
  def signed_amount_label
    sign = debit? ? "-" : "+"
    amt  = sprintf("%.2f", amount_cents.abs / 100.0)
    "#{sign}#{amt} #{currency}"
  end
end
