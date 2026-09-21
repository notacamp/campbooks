# frozen_string_literal: true

module Api
  module App
    module Money
      # Serializes a Loan record for the SPA's money/loans surface.
      class LoanSerializer
        def initialize(loan)
          @loan = loan
        end

        def as_json
          {
            id:                       @loan.id,
            lender:                   @loan.lender,
            source_counterparty:      @loan.source_counterparty,
            principal_cents:          @loan.principal_cents,
            instalment_cents:         @loan.instalment_cents,
            first_instalment_on:      @loan.first_instalment_on&.iso8601,
            # Exact next upcoming instalment date (status :expected, on/after today)
            # so the Books loan card needn't approximate from first + paid_count,
            # which drifts when an instalment was missed. nil once the loan is done.
            next_instalment_on:       @loan.next_expected&.expected_on&.iso8601,
            term_months:              @loan.term_months,
            rate_note:                @loan.rate_note,
            notes:                    @loan.notes,
            status:                   @loan.status,
            paid_count:               @loan.paid_count,
            remaining_count:          @loan.remaining_count,
            missed_count:             @loan.missed_instalments.count,
            change_acknowledged_at:   @loan.change_acknowledged_at&.iso8601,
            created_at:               @loan.created_at.iso8601,
            updated_at:               @loan.updated_at.iso8601
          }
        end
      end
    end
  end
end
