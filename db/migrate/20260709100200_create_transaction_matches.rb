# frozen_string_literal: true

class CreateTransactionMatches < ActiveRecord::Migration[8.1]
  # Links a BankTransaction to a Document (expense invoice, receipt, etc.).
  # One transaction can have multiple candidate matches; the user confirms one.
  def change
    create_table :transaction_matches, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :bank_transaction, null: false,
                   foreign_key: { on_delete: :cascade }, type: :uuid
      t.references :document, null: false,
                   foreign_key: { on_delete: :cascade }, type: :uuid

      t.integer :status,     null: false, default: 0     # suggested/confirmed/rejected
      t.float   :confidence                               # 0.0–1.0 from the matching engine
      t.integer :matched_by, null: false, default: 0     # heuristic/ai/manual
      t.jsonb   :match_reasons, null: false, default: {} # explanation hash for UI

      t.timestamps
    end

    # t.references :document already creates an index on document_id.
    # Add only the composite unique index.
    add_index :transaction_matches, [ :bank_transaction_id, :document_id ],
              unique: true, name: "index_transaction_matches_on_transaction_and_document"
  end
end
