class CreateEmailAccountsAndRestoreEmailTables < ActiveRecord::Migration[8.1]
  def change
    create_table :email_accounts do |t|
      t.string :email_address, null: false
      t.string :zoho_account_id
      t.string :zoho_refresh_token, null: false
      t.boolean :active, default: true, null: false
      t.datetime :last_scanned_at

      t.timestamps
    end

    add_index :email_accounts, :email_address, unique: true

    create_table :email_messages do |t|
      t.references :email_account, null: false, foreign_key: true
      t.string :zoho_message_id, null: false
      t.string :zoho_folder_id
      t.string :from_address
      t.string :subject
      t.datetime :received_at
      t.integer :status, null: false, default: 0

      t.timestamps
    end

    add_index :email_messages, [ :email_account_id, :zoho_message_id ], unique: true
    add_index :email_messages, :status

    create_table :email_scan_logs do |t|
      t.references :email_account, null: false, foreign_key: true
      t.integer :status, null: false, default: 0
      t.datetime :started_at
      t.datetime :completed_at
      t.integer :emails_found, default: 0
      t.integer :emails_processed, default: 0
      t.integer :documents_created, default: 0
      t.integer :errors_count, default: 0
      t.jsonb :error_messages, default: []

      t.timestamps
    end

    add_reference :documents, :email_account, foreign_key: true
  end
end
