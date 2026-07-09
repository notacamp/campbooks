# frozen_string_literal: true

class CreateReconciliations < ActiveRecord::Migration[8.1]
  # Accounting module — bank statement reconciliation.
  #
  # A Reconciliation is the workspace record that represents one imported bank
  # statement (the source Document) and the bookkeeping session that matches its
  # BankTransactions against expense/revenue Documents.  UUID PKs throughout,
  # matching the post-#89 convention.
  def change
    create_table :reconciliations, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.references :created_by, null: false,
                   foreign_key: { to_table: :users, on_delete: :restrict }, type: :uuid
      t.references :statement_document, null: false,
                   foreign_key: { to_table: :documents, on_delete: :restrict }, type: :uuid

      t.integer :status, null: false, default: 0       # pending/parsing/matching/ready/failed
      t.text    :parse_error

      t.date   :period_start
      t.date   :period_end
      t.string :bank_name
      t.string :currency, null: false, default: "EUR"

      # Optional balance bookends for integrity verification.
      t.integer :opening_balance_cents
      t.integer :closing_balance_cents

      t.boolean :integrity_warning,         null: false, default: false
      t.text    :integrity_warning_message

      t.integer :export_status, null: false, default: 0  # none/generating/generated/failed

      t.timestamps
    end

    # t.references above already creates single-column indexes on workspace_id,
    # statement_document_id, and created_by_id.  Only add the composite index
    # that t.references cannot create.
    add_index :reconciliations, [ :workspace_id, :status ],
              name: "index_reconciliations_on_workspace_and_status"
  end
end
