class CreateAuditEvents < ActiveRecord::Migration[8.1]
  def change
    create_table :audit_events do |t|
      # Nullify on user deletion so the audit record survives (anonymised) without
      # blocking erasure (Art. 17 vs. the audit trail).
      t.references :user, foreign_key: { on_delete: :nullify }, null: true
      t.string :action, null: false
      t.references :target, polymorphic: true, null: true
      t.string :ip_address
      t.string :user_agent
      t.jsonb :metadata, default: {}, null: false
      t.timestamps
    end

    add_index :audit_events, :action
    add_index :audit_events, :created_at
  end
end
