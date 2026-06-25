class AddUserCreatedIndexToAuditEvents < ActiveRecord::Migration[8.1]
  def change
    # Backs the per-user audit-log list (WHERE user_id = ? ORDER BY created_at
    # DESC) and the daily retention sweep (WHERE created_at < cutoff). The table
    # has no created_at index today, so both would otherwise sequential-scan.
    add_index :audit_events, [ :user_id, :created_at ]
  end
end
