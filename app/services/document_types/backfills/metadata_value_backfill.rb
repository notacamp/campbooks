# frozen_string_literal: true

module DocumentTypes
  module Backfills
    # Migration 3 helper: copies non-NULL values from the 23 legacy extracted-field
    # columns into the metadata JSONB column.
    #
    # Column values WIN over existing metadata (the edit form wrote to columns;
    # stale metadata loses). NULL columns leave the matching metadata key untouched.
    #
    # Conversions:
    #   String columns     → JSONB string
    #   Integer columns    → JSONB number
    #   numeric tax_rate   → JSONB number
    #   Date columns       → 'YYYY-MM-DD' string via to_char
    #   expense_category   → string key via SQL CASE (integer 0–9)
    #   company_vat_present → JSONB boolean (true/false, not null)
    #   currency           → JSONB string (has DB default 'EUR', never null in practice)
    #
    # Implementation: one UPDATE per batch using jsonb_strip_nulls(jsonb_build_object(…))
    # merged via || so that NULL columns produce no key (jsonb_strip_nulls drops JSON
    # nulls) and existing metadata keys not in the legacy set are preserved.
    #
    # Idempotent: the same non-NULL column value produces the same JSONB entry.
    class MetadataValueBackfill
      # Stub model that sees all 23 ignored columns.
      class MigDocument < ActiveRecord::Base
        self.table_name = "documents"
      end

      def self.run!
        new.run!
      end

      def run!
        conn = ActiveRecord::Base.connection

        MigDocument.in_batches(of: 500) do |batch|
          ids = batch.pluck(:id)
          next if ids.empty?

          quoted_ids = ids.map { |id| conn.quote(id.to_s) }.join(", ")

          conn.transaction do
            conn.execute(<<~SQL)
              UPDATE documents
              SET metadata = COALESCE(metadata, '{}') || jsonb_strip_nulls(jsonb_build_object(
                'vendor_name',           vendor_name,
                'vendor_nif',            vendor_nif,
                'client_name',           client_name,
                'client_nif',            client_nif,
                'buyer_nif',             buyer_nif,
                'bank_name',             bank_name,
                'sender_name',           sender_name,
                'account_number',        account_number,
                'invoice_number',        invoice_number,
                'receipt_number',        receipt_number,
                'payment_method',        payment_method,
                'amount_cents',          amount_cents,
                'tax_amount_cents',      tax_amount_cents,
                'opening_balance_cents', opening_balance_cents,
                'closing_balance_cents', closing_balance_cents,
                'tax_rate',              tax_rate,
                'document_date',         to_char(document_date, 'YYYY-MM-DD'),
                'due_date',              to_char(due_date, 'YYYY-MM-DD'),
                'period_start',          to_char(period_start, 'YYYY-MM-DD'),
                'period_end',            to_char(period_end, 'YYYY-MM-DD'),
                'expense_category', CASE expense_category
                  WHEN 0 THEN 'travel'
                  WHEN 1 THEN 'meals'
                  WHEN 2 THEN 'office_supplies'
                  WHEN 3 THEN 'utilities'
                  WHEN 4 THEN 'rent'
                  WHEN 5 THEN 'software'
                  WHEN 6 THEN 'professional_services'
                  WHEN 7 THEN 'equipment'
                  WHEN 8 THEN 'marketing'
                  WHEN 9 THEN 'other'
                END,
                'company_vat_present',   company_vat_present,
                'currency',              currency
              ))
              WHERE id IN (#{quoted_ids})
            SQL
          end
        end
      end
    end
  end
end
