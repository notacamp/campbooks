# frozen_string_literal: true

module Api
  module App
    module Money
      # Serializes a LoanInstalment row.
      class LoanInstalmentSerializer
        def initialize(instalment)
          @ins = instalment
        end

        def as_json
          {
            id:                    @ins.id,
            loan_id:               @ins.loan_id,
            expected_on:           @ins.expected_on&.iso8601,
            amount_cents:          @ins.amount_cents,
            status:                @ins.status,
            previous_amount_cents: @ins.previous_amount_cents,
            bank_transaction_id:   @ins.bank_transaction_id
          }
        end
      end
    end
  end
end
