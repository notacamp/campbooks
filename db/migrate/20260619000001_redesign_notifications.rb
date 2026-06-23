class RedesignNotifications < ActiveRecord::Migration[8.1]
  def change
    # Classification. Defaults backfill existing rows to activity/awaiting
    # (Postgres applies a constant default to existing rows on ADD COLUMN),
    # which is the right neutral bucket for the 4 legacy notification types.
    add_column :notifications, :category, :integer, null: false, default: 2 # activity
    add_column :notifications, :priority, :integer, null: false, default: 1 # awaiting

    # Lifecycle: archived = user-cleared (reversible); resolved = auto-cleared
    # when the underlying subject is resolved.
    add_column :notifications, :archived_at, :datetime
    add_column :notifications, :resolved_at, :datetime

    # Polymorphic subject for state-backed auto-resolve (nullable: activity
    # notifications have no subject).
    add_column :notifications, :notifiable_type, :string
    add_column :notifications, :notifiable_id, :bigint

    add_index :notifications, [ :user_id, :category, :resolved_at, :archived_at ],
              name: "idx_notifications_active_by_category"
    add_index :notifications, [ :notifiable_type, :notifiable_id ],
              name: "index_notifications_on_notifiable"
    add_index :notifications, :resolved_at
    add_index :notifications, :archived_at

    # NOTE: a unique partial index on (user_id, group_key) WHERE active is
    # introduced later, together with the single-active-row grouping semantics
    # and a one-time dedup of legacy windowed-duplicate rows. It cannot be added
    # here because the legacy 5-minute-window grouping produced multiple active
    # rows per group_key by design.
  end
end
