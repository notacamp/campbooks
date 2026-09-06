# frozen_string_literal: true

# A workspace-scoped standing explanation for a recurring bank debit that follows
# a fixed payment schedule (mortgage, business loan, car finance, leasing, etc.).
#
# Once tracked, the reconciler links each instalment to the bank line that paid it:
# the line reads "Loan · instalment N of T" on the statement, and Money shows what's
# paid, what's to go, the next instalment, and anything odd (a missing instalment, an
# amount change after a rate reset).
class Loan < ApplicationRecord
  belongs_to :workspace
  belongs_to :created_by, class_name: "User"
  has_many :instalments, class_name: "LoanInstalment", dependent: :destroy

  enum :status, { active: 0, closed: 1 }

  validates :lender,             presence: true
  validates :principal_cents,    numericality: { greater_than: 0 }
  validates :instalment_cents,   numericality: { greater_than: 0 }
  validates :term_months,        numericality: { greater_than: 0, only_integer: true }
  validates :first_instalment_on, presence: true
  validates :currency,           format: { with: /\A[A-Z]{3}\z/, message: "must be a 3-letter code" }

  scope :active_loans, -> { where(status: :active) }

  # ── Computed helpers ──────────────────────────────────────────────────────────

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

  def remaining_cents
    remaining_count * instalment_cents
  end

  def next_expected
    instalments.where(status: :expected).order(:expected_on).first
  end

  def last_seen
    instalments.where(status: %i[paid unverified]).where.not(bank_transaction_id: nil)
               .order(expected_on: :desc).first
  end

  def ends_on
    instalments.maximum(:expected_on)
  end

  # Returns the earliest paid instalment that has `previous_amount_cents` set AND
  # whose amount_cents equals the current `instalment_cents` — i.e. the point where
  # the rate reset landed at the current value.
  def amount_changed_at
    instalments.where.not(previous_amount_cents: nil)
               .where(amount_cents: instalment_cents, status: %i[paid unverified])
               .order(:expected_on).first
  end

  def progress_pct
    return 0 if term_months.zero?

    ((paid_count.to_f / term_months) * 100).round
  end

  # The lender name or token used for counterparty matching.
  def effective_counterparty
    source_counterparty.presence || lender
  end
end
