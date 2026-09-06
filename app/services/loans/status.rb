# frozen_string_literal: true

module Loans
  # Refreshes the status of non-paid instalments based on statement coverage.
  #
  # Rules (per instalment with status expected or missed):
  #   unverified — expected_on < earliest_covered_on (before your statements)
  #   missed     — expected_on + 12 days <= latest_covered_on AND not linked
  #   expected   — everything else
  #
  # Call after every Loans::Matcher run and after create/update.
  class Status
    # @param loan [Loan]
    # @param earliest_covered_on [Date, nil]   min period_start across ready reconciliations
    # @param latest_covered_on   [Date, nil]   max period_end across ready reconciliations
    def self.refresh!(loan, earliest_covered_on: nil, latest_covered_on: nil)
      new(loan, earliest_covered_on: earliest_covered_on,
                latest_covered_on:   latest_covered_on).call
    end

    def initialize(loan, earliest_covered_on: nil, latest_covered_on: nil)
      @loan = loan
      @earliest, @latest = resolve_coverage(earliest_covered_on, latest_covered_on)
    end

    def call
      unpaid = @loan.instalments.where(status: %i[expected missed]).to_a
      return if unpaid.empty?

      unpaid.each do |ins|
        new_status = determine_status(ins)
        ins.update_columns(status: LoanInstalment.statuses[new_status]) if ins.status != new_status.to_s
      end
    end

    private

    def resolve_coverage(earliest, latest)
      return [ earliest, latest ] if earliest.present? || latest.present?

      recs = @loan.workspace.reconciliations.where(status: :ready)
                  .where.not(period_start: nil).where.not(period_end: nil)
      return [ nil, nil ] if recs.empty?

      [ recs.minimum(:period_start), recs.maximum(:period_end) ]
    end

    def determine_status(instalment)
      date = instalment.expected_on

      if @earliest.present? && date < @earliest
        :unverified
      elsif @latest.present? && (date + 12.days) <= @latest
        :missed
      else
        :expected
      end
    end
  end
end
