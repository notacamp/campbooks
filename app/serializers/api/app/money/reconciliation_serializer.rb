# frozen_string_literal: true

module Api
  module App
    module Money
      # Serializes a Reconciliation (bank statement) header with its progress counts.
      # Optionally include statement_counts from Money::Page.
      class ReconciliationSerializer
        def initialize(reconciliation, resolved: nil, total: nil)
          @rec      = reconciliation
          @resolved = resolved
          @total    = total
        end

        def as_json
          {
            id:              @rec.id,
            status:          @rec.status,
            export_status:   @rec.export_status,
            bank_name:       @rec.bank_name,
            currency:        @rec.currency,
            document_id:     @rec.statement_document_id,
            document_title:  @rec.statement_document&.display_title,
            resolved_count:  @resolved.nil? ? @rec.resolved_count : @resolved,
            total_count:     @total.nil? ? @rec.total_transactions : @total,
            created_at:      @rec.created_at.iso8601,
            updated_at:      @rec.updated_at.iso8601
          }
        end
      end
    end
  end
end
