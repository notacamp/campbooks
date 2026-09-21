# frozen_string_literal: true

module Api
  module App
    module Money
      # Serializes the full Money::Page read model — the single response shape that
      # every money mutation also returns (the `surface=money` contract for the SPA).
      class PageSerializer
        def initialize(page)
          @page = page
        end

        def as_json
          {
            obligations:      serialize_obligations,
            needs_you:        serialize_needs_you,
            needs_you_count:  @page.needs_you.size,
            loans:            serialize_loans,
            loan_suggestions: serialize_loan_suggestions,
            statement_counts: serialize_statement_counts,
            selected_statement_id: @page.selected_statement&.id
          }
        end

        private

        def serialize_obligations
          @page.ledger.obligations.map { |ob| ObligationSerializer.new(ob).as_json }
        end

        def serialize_needs_you
          @page.needs_you.map { |item| NeedsYouItemSerializer.new(item).as_json }
        end

        def serialize_loans
          @page.loans.map { |loan| LoanSerializer.new(loan).as_json }
        end

        def serialize_loan_suggestions
          Array(@page.loan_suggestions).map do |s|
            {
              key:          s.respond_to?(:key)          ? s.key          : nil,
              lender:       s.respond_to?(:lender)       ? s.lender       : nil,
              amount_cents: s.respond_to?(:amount_cents) ? s.amount_cents : nil,
              counterparty: s.respond_to?(:counterparty) ? s.counterparty : nil
            }
          end
        end

        def serialize_statement_counts
          @page.statement_counts.transform_keys(&:to_s)
        end
      end
    end
  end
end
