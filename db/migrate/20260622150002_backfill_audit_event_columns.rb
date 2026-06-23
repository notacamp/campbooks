class BackfillAuditEventColumns < ActiveRecord::Migration[8.1]
  # Forward-only, idempotent: the original CreateAuditEvents registered as `up`
  # with only its timestamps (shared-Postgres version collision), so backfill the
  # real columns here instead of fighting an unreversible redo.
  def up
    unless column_exists?(:audit_events, :user_id)
      add_reference :audit_events, :user, foreign_key: { on_delete: :nullify }, null: true
    end
    add_column :audit_events, :action, :string unless column_exists?(:audit_events, :action)
    unless column_exists?(:audit_events, :target_id)
      add_reference :audit_events, :target, polymorphic: true, null: true
    end
    add_column :audit_events, :ip_address, :string unless column_exists?(:audit_events, :ip_address)
    add_column :audit_events, :user_agent, :string unless column_exists?(:audit_events, :user_agent)
    unless column_exists?(:audit_events, :metadata)
      add_column :audit_events, :metadata, :jsonb, default: {}, null: false
    end
    add_index :audit_events, :action unless index_exists?(:audit_events, :action)
  end

  def down
    # no-op: forward fix-up for a partially-applied create.
  end
end
