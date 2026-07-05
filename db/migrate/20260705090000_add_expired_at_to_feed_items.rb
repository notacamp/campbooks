class AddExpiredAtToFeedItems < ActiveRecord::Migration[8.1]
  # Reconcile used to stamp acted_at when a card stopped qualifying, which (a)
  # conflated system-expiry with a real user action — poisoning acted_at as an
  # engagement signal — and (b) could never be undone by the generator's upsert
  # (acted_at isn't in update_only), so a card that re-qualified later (a snooze
  # expiring again, a follow-up falling due again) stayed invisible forever.
  #
  # System-expiry now lives in its own column: reconcile sets expired_at, the
  # upsert clears it when the record re-qualifies, and the active-row partial
  # indexes carry the extra predicate. No backfill — NULL ("never expired") is
  # correct for every existing row; rows already resolved via the old acted_at
  # path stay resolved and age out. Scores/flags self-heal on the next
  # generation run (15-min sweep / debounced read refresh).
  def change
    add_column :feed_items, :expired_at, :datetime

    remove_index :feed_items, name: "idx_feed_items_attention"
    add_index :feed_items, %i[user_id score sort_at],
              name: "idx_feed_items_attention",
              order: { score: :desc, sort_at: :desc },
              where: "dismissed_at IS NULL AND acted_at IS NULL AND expired_at IS NULL AND attention = true"

    remove_index :feed_items, name: "idx_feed_items_timeline"
    add_index :feed_items, %i[user_id score sort_at],
              name: "idx_feed_items_timeline",
              order: { score: :desc, sort_at: :desc },
              where: "dismissed_at IS NULL AND acted_at IS NULL AND expired_at IS NULL AND attention = false"

    remove_index :feed_items, name: "index_feed_items_on_user_unseen_active"
    add_index :feed_items, [ :user_id ],
              name: "index_feed_items_on_user_unseen_active",
              where: "seen_at IS NULL AND dismissed_at IS NULL AND acted_at IS NULL AND expired_at IS NULL"
  end
end
