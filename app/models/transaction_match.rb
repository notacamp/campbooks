# frozen_string_literal: true

# A candidate or confirmed pairing between a BankTransaction and a Document.
# The matching engine (PR 2) creates `suggested` matches; the user confirms or
# rejects them, or creates `manual` matches from the workbench.
#
# `allocated_cents` records the portion of the matched Document's amount that
# this specific bank transaction covers. For a straightforward 1:1 full payment
# it equals the document's total amount_cents. For split or installment scenarios
# it may be a fraction. nil means "unknown / not yet set".
class TransactionMatch < ApplicationRecord
  belongs_to :bank_transaction
  belongs_to :document

  enum :status, {
    suggested: 0,
    confirmed: 1,
    rejected:  2
  }

  enum :matched_by, {
    heuristic: 0,
    ai:        1,
    manual:    2
  }

  validates :document_id, uniqueness: { scope: :bank_transaction_id }
  validates :confidence, numericality: { greater_than_or_equal_to: 0.0, less_than_or_equal_to: 1.0 },
                         allow_nil: true
  validates :allocated_cents, numericality: { greater_than_or_equal_to: 0 }, allow_nil: true

  # When allocated_cents is not supplied at create time, default to the matched
  # document's full amount (preserving the legacy full-settlement semantics).
  before_create :default_allocated_cents

  # The single place a bank reconciliation writes "paid" back onto a Document (Paper's
  # `paid` status). Any change to a match — confirmed, rejected, reset, deleted —
  # re-derives the document's bank-match settlement from ALL its confirmed matches, so
  # settled_at is right whether this was the first confirmation or the last one undone.
  after_commit :sync_document_settlement

  private

  def default_allocated_cents
    self.allocated_cents ||= document.amount_cents
  end

  def sync_document_settlement
    # The document may already be gone (dependent: :destroy cascade); nothing to sync.
    Document.find_by(id: document_id)&.recompute_bank_settlement!
  end
end
