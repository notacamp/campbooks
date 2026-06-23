class CreateEmailAccountSignatures < ActiveRecord::Migration[8.1]
  def change
    create_table :email_account_signatures do |t|
      t.references :signature, null: false, foreign_key: true
      t.references :email_account, null: false, foreign_key: true
      t.timestamps
    end

    add_index :email_account_signatures, [ :signature_id, :email_account_id ], unique: true
  end
end
