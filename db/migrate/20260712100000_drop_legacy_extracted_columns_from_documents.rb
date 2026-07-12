# frozen_string_literal: true

# Final step of the schema-driven document types migration (v0.24.0 unified the
# storage): the 23 legacy extracted-value columns have been dead since the value
# backfill copied them into the documents.metadata JSONB — every reader and
# writer goes through metadata now. Dropping them (and their indexes) completes
# the move. Safe to run on any upgrade path: the v0.24.0 backfill migrations run
# first (idempotently) on the same boot.
class DropLegacyExtractedColumnsFromDocuments < ActiveRecord::Migration[8.1]
  LEGACY_COLUMNS = %w[
    vendor_name vendor_nif client_name client_nif buyer_nif bank_name
    sender_name account_number invoice_number receipt_number payment_method
    amount_cents tax_amount_cents tax_rate opening_balance_cents
    closing_balance_cents currency document_date due_date period_start
    period_end expense_category company_vat_present
  ].freeze

  def up
    remove_index :documents, :document_date, if_exists: true
    remove_index :documents, :vendor_nif, if_exists: true
    remove_index :documents, :client_nif, if_exists: true
    remove_index :documents, name: "index_documents_on_workspace_and_due_date", if_exists: true

    LEGACY_COLUMNS.each do |column|
      remove_column :documents, column, if_exists: true
    end
  end

  def down
    raise ActiveRecord::IrreversibleMigration,
          "Legacy extracted-value columns cannot be restored — values live in documents.metadata"
  end
end
