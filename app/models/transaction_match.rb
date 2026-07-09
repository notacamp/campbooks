# frozen_string_literal: true

# A candidate or confirmed pairing between a BankTransaction and a Document.
# The matching engine (PR 2) creates `suggested` matches; the user confirms or
# rejects them, or creates `manual` matches from the workbench.
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
end
