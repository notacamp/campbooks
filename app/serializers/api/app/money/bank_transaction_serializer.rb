# frozen_string_literal: true

module Api
  module App
    module Money
      # Serializes a BankTransaction with its match suggestions and current status.
      class BankTransactionSerializer
        def initialize(transaction)
          @txn = transaction
        end

        def as_json
          {
            id:               @txn.id,
            reconciliation_id: @txn.reconciliation_id,
            status:           @txn.status,
            position:         @txn.position,
            booked_on:        @txn.booked_on&.iso8601,
            description:      @txn.description,
            counterparty:     @txn.counterparty,
            amount_cents:     @txn.amount_cents,
            currency:         @txn.currency,
            exclusion_reason: @txn.exclusion_reason,
            requested_at:     @txn.requested_at&.iso8601,
            resolved:         BankTransaction::RESOLVED_STATUSES.include?(@txn.status.to_sym),
            loan_explained:   @txn.explained?,
            matches:          serialize_matches
          }
        end

        private

        def serialize_matches
          @txn.transaction_matches.map do |m|
            {
              id:          m.id,
              status:      m.status,
              confidence:  m.confidence,
              matched_by:  m.matched_by,
              document_id: m.document_id,
              document_title: m.document&.display_title
            }
          end
        end
      end
    end
  end
end
