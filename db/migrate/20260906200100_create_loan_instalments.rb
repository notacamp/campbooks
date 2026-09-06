# frozen_string_literal: true

class CreateLoanInstalments < ActiveRecord::Migration[8.1]
  def change
    create_table :loan_instalments, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.uuid    :loan_id,                    null: false
      t.integer :number,                     null: false
      t.date    :expected_on,                null: false
      t.bigint  :amount_cents,               null: false
      t.bigint  :previous_amount_cents
      t.uuid    :bank_transaction_id
      t.integer :status,                     null: false, default: 0
      t.string  :note

      t.timestamps
    end

    add_index :loan_instalments, :loan_id
    add_index :loan_instalments, :bank_transaction_id, unique: true
    add_index :loan_instalments, %i[loan_id number], unique: true

    add_foreign_key :loan_instalments, :loans
    add_foreign_key :loan_instalments, :bank_transactions
  end
end
