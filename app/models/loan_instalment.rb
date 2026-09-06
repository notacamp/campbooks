# frozen_string_literal: true

# One expected (or past) monthly payment of a Loan. Linked to the BankTransaction
# that paid it once the reconciler finds a matching bank line.
class LoanInstalment < ApplicationRecord
  belongs_to :loan
  belongs_to :bank_transaction, optional: true

  enum :status, {
    expected:   0,
    paid:       1,
    missed:     2,
    unverified: 3  # before the earliest uploaded statement — assumed paid
  }

  scope :ordered,       -> { order(:number) }
  scope :by_status,     ->(s) { where(status: s) }
  scope :not_paid,      -> { where(status: %i[expected missed]) }
  scope :with_bank_txn, -> { where.not(bank_transaction_id: nil) }

  validates :number,       presence: true, numericality: { only_integer: true, greater_than: 0 }
  validates :expected_on,  presence: true
  validates :amount_cents, presence: true, numericality: { greater_than: 0 }

  # When an instalment is destroyed, reset the linked bank transaction back to
  # unmatched so it appears in Needs you again.
  after_destroy :reset_bank_transaction_if_explained

  # When the link is cleared (bank_transaction_id set to nil), also reset the txn.
  before_update :reset_bank_transaction_if_link_cleared, if: :bank_transaction_id_changed?

  private

  def reset_bank_transaction_if_explained
    return unless bank_transaction_id.present?

    txn = BankTransaction.find_by(id: bank_transaction_id)
    txn&.update_columns(status: BankTransaction.statuses[:unmatched]) if txn&.explained?
  end

  def reset_bank_transaction_if_link_cleared
    return unless bank_transaction_id_was.present? && bank_transaction_id.nil?

    txn = BankTransaction.find_by(id: bank_transaction_id_was)
    txn&.update_columns(status: BankTransaction.statuses[:unmatched]) if txn&.explained?
  end
end
