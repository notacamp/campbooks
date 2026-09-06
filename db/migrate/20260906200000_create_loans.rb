# frozen_string_literal: true

class CreateLoans < ActiveRecord::Migration[8.1]
  def change
    create_table :loans, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.uuid     :workspace_id,               null: false
      t.uuid     :created_by_id,              null: false
      t.string   :lender,                     null: false
      t.string   :source_counterparty
      t.bigint   :principal_cents,            null: false
      t.string   :currency,                   null: false, default: "EUR"
      t.bigint   :instalment_cents,           null: false
      t.date     :first_instalment_on,        null: false
      t.integer  :term_months,                null: false
      t.string   :rate_note
      t.text     :notes
      t.integer  :status,                     null: false, default: 0
      t.datetime :change_acknowledged_at

      t.timestamps
    end

    add_index :loans, :workspace_id
    add_index :loans, %i[workspace_id status]
    add_foreign_key :loans, :workspaces
    add_foreign_key :loans, :users, column: :created_by_id
  end
end
