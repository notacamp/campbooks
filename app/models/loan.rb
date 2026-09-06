# frozen_string_literal: true

# A workspace-scoped standing explanation for a recurring bank debit on a fixed
# schedule (a business loan, a mortgage, car finance, leasing).
#
# Once tracked, the reconciler links each instalment to the bank line that paid it:
# the line reads "Loan · instalment N of T" on the statement, and Money shows what's
# paid, what's to go, the next instalment, and anything odd (a missing instalment,
# an amount change after a rate reset). Loans::Status keeps the derived state honest.
class Loan < ApplicationRecord
  belongs_to :workspace
  belongs_to :created_by, class_name: "User"
  has_many :instalments, class_name: "LoanInstalment", dependent: :destroy

  enum :status, { active: 0, closed: 1 }

  validates :lender,              presence: true
  validates :principal_cents,     numericality: { greater_than: 0 }
  validates :instalment_cents,    numericality: { greater_than: 0 }
  validates :term_months,         numericality: { greater_than: 0, only_integer: true }
  validates :first_instalment_on, presence: true
  validates :currency,            format: { with: /\A[A-Z]{3}\z/, message: "must be a 3-letter code" }

  scope :active_loans, -> { where(status: :active) }

  # ── What the statements prove ────────────────────────────────────────────────

  # Paid on a statement, or dated before the first statement (assumed paid).
  def paid_instalments
    instalments.where(status: %i[paid unverified])
  end

  def paid_count
    paid_instalments.count
  end

  def paid_cents
    paid_instalments.sum(:amount_cents)
  end

  def remaining_instalments
    instalments.where(status: %i[expected missed])
  end

  def remaining_count
    remaining_instalments.count
  end

  # What is left to pay at the current instalment (honest without a rate).
  def remaining_cents
    remaining_count * instalment_cents
  end

  def missed_instalments
    instalments.where(status: :missed).order(:expected_on)
  end

  # The next instalment still to come (today or later).
  def next_expected(today = Date.current)
    instalments.where(status: :expected).where("expected_on >= ?", today).order(:expected_on).first
  end

  # Instalments whose date has passed with no statement covering them yet: they
  # are neither proven paid nor missed. Newest last.
  def awaiting_statement(today = Date.current)
    instalments.where(status: :expected).where("expected_on < ?", today).order(:expected_on)
  end

  # The latest instalment found on a statement.
  def last_seen
    instalments.where(status: :paid).where.not(bank_transaction_id: nil).order(expected_on: :desc).first
  end

  # The latest statement-proven instalments, newest first.
  def recent_paid(limit = 3)
    instalments.where(status: :paid).where.not(bank_transaction_id: nil)
               .includes(bank_transaction: :reconciliation)
               .order(expected_on: :desc).limit(limit)
  end

  def ends_on
    instalments.maximum(:expected_on)
  end

  # The most recent instalment whose amount stepped from the one before it
  # (Loans::Status sets previous_amount_cents); nil when the amount never changed.
  def amount_changed_at
    instalments.where(status: :paid).where.not(previous_amount_cents: nil).order(expected_on: :desc).first
  end

  # The amount change the user hasn't waved through yet.
  def unacknowledged_change
    changed = amount_changed_at
    return nil unless changed
    return changed if change_acknowledged_at.nil?

    changed if change_acknowledged_at < changed.expected_on.in_time_zone
  end

  def progress_pct
    return 0 if term_months.zero?

    ((paid_count.to_f / term_months) * 100).round
  end

  # What the statement calls the lender, for matching.
  def effective_counterparty
    source_counterparty.presence || lender
  end
end
