# frozen_string_literal: true

module Loans
  # Builds or rebuilds the instalment schedule for a Loan.
  #
  # Instalment n (1-based) falls on first_instalment_on >> (n-1).
  # Ruby's Date#>> advances by n months, clamping to month-end automatically.
  class Schedule
    # Create all `term_months` instalments for a brand-new loan.
    # Caller is responsible for transactional wrapping.
    def self.build!(loan)
      instalments = (1..loan.term_months).map do |n|
        {
          loan_id:      loan.id,
          number:       n,
          expected_on:  loan.first_instalment_on >> (n - 1),
          amount_cents: loan.instalment_cents,
          status:       LoanInstalment.statuses[:expected],
          created_at:   Time.current,
          updated_at:   Time.current
        }
      end
      LoanInstalment.insert_all!(instalments)
    end

    # Rebuild schedule after a term change. Paid/unverified rows are untouched;
    # expected/missed rows are deleted and regenerated so numbering stays consecutive
    # and the total count == term_months.
    def self.rebuild!(loan)
      loan.instalments.where(status: %i[expected missed]).delete_all

      paid_numbers = loan.instalments.where(status: %i[paid unverified])
                         .pluck(:number).to_set

      instalments = (1..loan.term_months).reject { |n| paid_numbers.include?(n) }.map do |n|
        {
          loan_id:      loan.id,
          number:       n,
          expected_on:  loan.first_instalment_on >> (n - 1),
          amount_cents: loan.instalment_cents,
          status:       LoanInstalment.statuses[:expected],
          created_at:   Time.current,
          updated_at:   Time.current
        }
      end

      LoanInstalment.insert_all!(instalments) if instalments.any?
    end
  end
end
