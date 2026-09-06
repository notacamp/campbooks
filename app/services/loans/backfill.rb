# frozen_string_literal: true

module Loans
  # Runs Loans::Matcher over the workspace's most recent ready reconciliations
  # so a newly tracked loan immediately claims its past instalments.
  # Then refreshes the loan's status.
  class Backfill
    MAX_RECONCILIATIONS = 36

    # @param loan [Loan]
    def self.call(loan)
      new(loan).call
    end

    def initialize(loan)
      @loan      = loan
      @workspace = loan.workspace
    end

    def call
      recent_reconciliations.each do |reconciliation|
        Loans::Matcher.new(reconciliation).call
      rescue => e
        Rails.logger.warn("[Loans::Backfill] matcher error on #{reconciliation.id}: #{e.class}: #{e.message}")
      end

      Loans::Status.refresh!(@loan)
    end

    private

    def recent_reconciliations
      @workspace.reconciliations
                .where(status: :ready)
                .order(created_at: :desc)
                .limit(MAX_RECONCILIATIONS)
    end
  end
end
