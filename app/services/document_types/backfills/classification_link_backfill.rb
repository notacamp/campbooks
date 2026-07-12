# frozen_string_literal: true

module DocumentTypes
  module Backfills
    # Migration 2 helper: sets document_type_id on documents whose legacy integer
    # document_type enum matches a DocumentType row by name in the same workspace.
    #
    # Only touches rows where document_type_id IS NULL. Rows that already have an
    # id (or whose enum value maps to no DocumentType in the workspace) are left
    # alone. Idempotent.
    #
    # Storage note: documents.document_type is an INTEGER column (default: 0).
    # The enum mapping is taken from Document::enum :document_type.
    class ClassificationLinkBackfill
      # Stub model: sees all columns including those in Document::IGNORED_COLUMN_NAMES.
      class MigDocument < ActiveRecord::Base
        self.table_name = "documents"
      end

      ENUM_TO_NAME = {
        0  => "expense_invoice",
        1  => "revenue_invoice",
        2  => "bank_statement",
        3  => "receipt",
        4  => "other",
        5  => "insurance_policy",
        6  => "vehicle_document",
        7  => "contract",
        8  => "certificate",
        9  => "tax_document",
        10 => "identification",
        11 => "proposal",
        12 => "correspondence",
        13 => "bank_journal_entry",
        14 => "credit_note"
      }.freeze

      def self.run!
        new.run!
      end

      def run!
        conn = ActiveRecord::Base.connection

        # Iterate ALL documents in batches; the WHERE document_type_id IS NULL
        # guard lives in the UPDATE so the cursor advances cleanly regardless.
        MigDocument.in_batches(of: 500) do |batch|
          ids = batch.pluck(:id)
          next if ids.empty?

          quoted_ids = ids.map { |id| conn.quote(id.to_s) }.join(", ")

          conn.transaction do
            conn.execute(<<~SQL)
              UPDATE documents d
              SET document_type_id = dt.id
              FROM document_types dt
              WHERE d.id IN (#{quoted_ids})
                AND d.document_type_id IS NULL
                AND d.workspace_id = dt.workspace_id
                AND dt.name = CASE d.document_type
                  WHEN 0  THEN 'expense_invoice'
                  WHEN 1  THEN 'revenue_invoice'
                  WHEN 2  THEN 'bank_statement'
                  WHEN 3  THEN 'receipt'
                  WHEN 4  THEN 'other'
                  WHEN 5  THEN 'insurance_policy'
                  WHEN 6  THEN 'vehicle_document'
                  WHEN 7  THEN 'contract'
                  WHEN 8  THEN 'certificate'
                  WHEN 9  THEN 'tax_document'
                  WHEN 10 THEN 'identification'
                  WHEN 11 THEN 'proposal'
                  WHEN 12 THEN 'correspondence'
                  WHEN 13 THEN 'bank_journal_entry'
                  WHEN 14 THEN 'credit_note'
                  ELSE NULL
                END
            SQL
          end
        end
      end
    end
  end
end
