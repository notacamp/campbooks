class CreateEmailAccountUsers < ActiveRecord::Migration[8.1]
  def change
    create_table :email_account_users do |t|
      t.references :email_account, null: false, foreign_key: true
      t.references :user, null: false, foreign_key: true
      t.boolean :can_read, default: true, null: false
      t.boolean :can_send, default: false, null: false
      t.boolean :can_manage, default: false, null: false
      t.boolean :owner, default: false, null: false

      t.timestamps
    end
    add_index :email_account_users, [ :email_account_id, :user_id ], unique: true
  end
end
