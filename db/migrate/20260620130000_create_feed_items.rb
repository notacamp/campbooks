class CreateFeedItems < ActiveRecord::Migration[8.1]
  def change
    create_table :feed_items do |t|
      # The feed is per-user; workspace is carried for scoping + cleanup.
      t.references :user, null: false, foreign_key: true, index: false
      t.references :workspace, null: false, foreign_key: true

      t.string :kind, null: false
      # Polymorphic pointer to the record the card is about (EmailMessage / Document).
      # Content is rendered live from this record — the row only materializes routing.
      t.references :subject, polymorphic: true, null: false, index: false

      # Stable identity for a logical item, e.g. "email_action:123". The unique
      # (user_id, dedupe_key) index makes generation an idempotent upsert.
      t.string :dedupe_key, null: false

      t.datetime :sort_at, null: false               # timeline position
      t.integer  :score, null: false, default: 0     # ranks the attention cluster
      t.boolean  :attention, null: false, default: false # true ⇒ pinned, false ⇒ timeline
      t.jsonb    :data, null: false, default: {}      # small render extras (cached)

      # Per-user lifecycle.
      t.datetime :seen_at
      t.datetime :dismissed_at
      t.datetime :acted_at
      t.datetime :generated_at # last time a source refreshed this row

      t.timestamps
    end

    # Idempotent upsert key — also covers `WHERE user_id = ?` lookups (leading column).
    add_index :feed_items, [ :user_id, :dedupe_key ], unique: true,
              name: "idx_feed_items_user_dedupe"

    # Timeline read: active, non-attention, reverse-chronological.
    add_index :feed_items, [ :user_id, :sort_at ],
              order: { sort_at: :desc },
              where: "dismissed_at IS NULL AND acted_at IS NULL AND attention = false",
              name: "idx_feed_items_timeline"

    # Attention cluster: active, attention, by score then recency.
    add_index :feed_items, [ :user_id, :score, :sort_at ],
              order: { score: :desc, sort_at: :desc },
              where: "dismissed_at IS NULL AND acted_at IS NULL AND attention = true",
              name: "idx_feed_items_attention"

    # Resolve items when their subject is handled elsewhere.
    add_index :feed_items, [ :subject_type, :subject_id ],
              name: "idx_feed_items_subject"
  end
end
