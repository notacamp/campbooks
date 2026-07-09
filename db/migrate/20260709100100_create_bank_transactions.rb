# frozen_string_literal: true

class CreateBankTransactions < ActiveRecord::Migration[8.1]
  # A BankTransaction is one row from an imported bank statement.
  # amount_cents is SIGNED: negative = debit (money out), positive = credit (money in).
  def change
    create_table :bank_transactions, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :reconciliation, null: false,
                   foreign_key: { on_delete: :cascade }, type: :uuid
      t.references :workspace, null: false, foreign_key: true, type: :uuid

      t.integer :position,        null: false             # original row order
      t.date    :booked_on,       null: false
      t.text    :description,     null: false
      t.string  :counterparty                             # payee / sender name when available
      t.integer :amount_cents,    null: false             # signed: negative=debit
      t.string  :currency,        null: false, default: "EUR"
      t.integer :balance_after_cents                      # running balance from statement

      t.jsonb   :raw_data,        null: false, default: {} # original parsed row hash

      t.integer :status,          null: false, default: 0  # unmatched/suggested/matched/excluded/requested
      t.string  :exclusion_reason

      # "Request invoice" flow: tracks who asked for this transaction's invoice and when.
      t.references :requested_by, foreign_key: { to_table: :users, on_delete: :nullify },
                   type: :uuid
      t.datetime :requested_at

      t.timestamps
    end

    add_index :bank_transactions, [ :reconciliation_id, :position ],
              unique: true, name: "index_bank_transactions_on_reconciliation_position"
    add_index :bank_transactions, [ :workspace_id, :status ],
              name: "index_bank_transactions_on_workspace_and_status"
  end
end
