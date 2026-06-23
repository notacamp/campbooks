class CreateZohoDriveAccounts < ActiveRecord::Migration[8.1]
  def change
    create_table :zoho_drive_accounts do |t|
      t.string :email_address, null: false
      t.string :zoho_account_id
      t.text :zoho_refresh_token, null: false
      t.boolean :active, default: true, null: false
      t.datetime :last_synced_at
      t.timestamps
    end

    add_index :zoho_drive_accounts, :email_address, unique: true
  end
end
