# frozen_string_literal: true

module Api
  module App
    module Money
      # Serializes a Money::NeedsYouItem — the priority action cards on the Money surface.
      class NeedsYouItemSerializer
        def initialize(item)
          @item = item
        end

        def as_json
          base = {
            kind:    @item.kind,
            title:   @item.title,
            meta:    Array(@item.meta).filter_map { |m| m.is_a?(Hash) ? m : m.to_s },
            actions: Array(@item.actions)
          }

          base[:transaction_id] = @item.transaction&.id if @item.transaction
          base[:match_id]       = @item.match&.id if @item.match
          base.merge(loan_payload)
        end

        private

        def loan_payload
          return {} unless @item.payload

          case @item.kind
          when :loan_missed, :loan_changed
            payload = @item.payload
            {
              loan_id:        payload[:loan]&.id,
              instalment_id:  payload[:instalment]&.id
            }
          when :loan_suggestion
            suggestion = @item.payload
            {
              suggestion_key:         suggestion.respond_to?(:key) ? suggestion.key : nil,
              suggestion_lender:      suggestion.respond_to?(:lender) ? suggestion.lender : nil,
              suggestion_amount_cents: suggestion.respond_to?(:amount_cents) ? suggestion.amount_cents : nil
            }
          when :statement_failed
            { reconciliation_id: @item.payload[:reconciliation]&.id }
          when :reconcile_statements
            {
              pending_count:    @item.payload[:count],
              document_ids:     Array(@item.payload[:documents]).map(&:id)
            }
          when :add_statement
            {
              focus_month:  @item.payload[:month]&.iso8601,
              focus_label:  @item.payload[:label]
            }
          else
            {}
          end
        end
      end
    end
  end
end
