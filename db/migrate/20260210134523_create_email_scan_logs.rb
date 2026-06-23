class CreateEmailScanLogs < ActiveRecord::Migration[8.1]
  def change
    create_table :email_scan_logs do |t|
      t.datetime :started_at
      t.datetime :completed_at
      t.integer :emails_found, default: 0
      t.integer :emails_processed, default: 0
      t.integer :documents_created, default: 0
      t.integer :errors_count, default: 0
      t.integer :status, null: false, default: 0
      t.jsonb :error_messages, default: []

      t.timestamps
    end
  end
end
