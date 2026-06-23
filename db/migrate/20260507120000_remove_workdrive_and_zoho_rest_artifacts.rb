class RemoveWorkdriveAndZohoRestArtifacts < ActiveRecord::Migration[8.1]
  def change
    remove_column :documents, :workdrive_folder, :string
    remove_column :documents, :workdrive_file_id, :string
    remove_column :documents, :uploaded_to_workdrive_at, :datetime

    drop_table :email_messages do |t|
      t.string :zoho_message_id, null: false
      t.string :zoho_folder_id
      t.string :from_address
      t.string :subject
      t.datetime :received_at
      t.integer :status, default: 0, null: false
      t.timestamps
      t.index [ :status ], name: "index_email_messages_on_status"
      t.index [ :zoho_message_id ], name: "index_email_messages_on_zoho_message_id", unique: true
    end

    drop_table :email_scan_logs do |t|
      t.integer :status, default: 0, null: false
      t.datetime :started_at
      t.datetime :completed_at
      t.integer :emails_found, default: 0
      t.integer :emails_processed, default: 0
      t.integer :documents_created, default: 0
      t.integer :errors_count, default: 0
      t.jsonb :error_messages, default: []
      t.timestamps
    end
  end
end
