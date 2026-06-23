class CreateCalendarSyncLogs < ActiveRecord::Migration[8.1]
  def change
    # Audit trail for each sync run — mirror of `email_scan_logs`.
    create_table :calendar_sync_logs do |t|
      t.references :calendar_account, null: false, foreign_key: true

      t.integer :status, null: false, default: 0 # running: 0, completed: 1, failed: 2
      t.datetime :started_at
      t.datetime :completed_at

      t.integer :events_found, default: 0
      t.integer :events_upserted, default: 0
      t.integer :errors_count, default: 0
      t.jsonb   :error_messages, default: []

      t.timestamps
    end
  end
end
