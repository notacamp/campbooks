# frozen_string_literal: true

module Loans
  # Re-derives what the statements prove about a loan. Two passes:
  #
  # 1. Amounts. Walking the bank-proven (paid) instalments in date order, an
  #    instalment whose amount differs from the previous one by 0.5% or more records
  #    that previous amount (a rate reset). The latest paid amount becomes the loan's
  #    current instalment and the amount of every instalment still to come. Deriving
  #    this from the sequence, rather than at link time, keeps it right no matter
  #    which statement was reconciled first.
  #
  # 2. Statuses, for instalments that are not paid:
  #      unverified  expected_on is before the earliest reconciled statement
  #                  (before your statements; assumed paid)
  #      missed      a reconciled statement covers expected_on + 12 days (the
  #                  latest plausible booking) and no bank line was found
  #      expected    everything else, including months with no statement yet
  #
  # Call after every Loans::Matcher run and after create/update.
  class Status
    RATE_STEP  = 0.005 # 0.5%
    GRACE_DAYS = 12

    def self.refresh!(loan, earliest_covered_on: nil, latest_covered_on: nil)
      new(loan, earliest_covered_on: earliest_covered_on, latest_covered_on: latest_covered_on).call
    end

    def initialize(loan, earliest_covered_on: nil, latest_covered_on: nil)
      @loan = loan
      @periods = ready_periods
      @earliest = earliest_covered_on || @periods.map(&:first).min
      @latest   = latest_covered_on   || @periods.map(&:last).max
    end

    def call
      sync_amounts!
      refresh_statuses!
    end

    private

    def sync_amounts!
      paid = @loan.instalments.where(status: :paid).order(:expected_on).to_a
      previous = nil

      paid.each do |ins|
        step = previous && (ins.amount_cents - previous).abs.to_f / previous >= RATE_STEP ? previous : nil
        ins.update_columns(previous_amount_cents: step) if ins.previous_amount_cents != step
        previous = ins.amount_cents
      end

      return if previous.nil? || previous == @loan.instalment_cents

      @loan.update_columns(instalment_cents: previous)
      @loan.instalments.where(status: %i[expected missed]).update_all(amount_cents: previous)
    end

    def refresh_statuses!
      @loan.instalments.where(status: %i[expected missed unverified]).find_each do |ins|
        new_status = status_for(ins)
        ins.update_columns(status: LoanInstalment.statuses[new_status]) if ins.status != new_status.to_s
      end
    end

    # [[period_start, period_end], …] of the workspace's reconciled statements.
    def ready_periods
      @loan.workspace.reconciliations.where(status: :ready)
           .where.not(period_start: nil).where.not(period_end: nil)
           .pluck(:period_start, :period_end)
    end

    def covered?(date)
      @periods.any? { |from, to| from <= date && date <= to } ||
        (@earliest.present? && @latest.present? && @periods.empty? && @earliest <= date && date <= @latest)
    end

    def status_for(instalment)
      date = instalment.expected_on

      if @earliest.present? && date < @earliest
        :unverified
      elsif covered?(date + GRACE_DAYS.days)
        :missed
      else
        :expected
      end
    end
  end
end
