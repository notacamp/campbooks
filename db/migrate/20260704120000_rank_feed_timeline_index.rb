class RankFeedTimelineIndex < ActiveRecord::Migration[8.1]
  # The home-feed timeline is now read in rank order (score DESC, sort_at DESC)
  # instead of purely chronologically, so the partial timeline index leads with
  # score — mirroring the attention index. Scores themselves self-heal: the next
  # Feed generation run (15-min sweep / debounced read refresh) rewrites every
  # active row with the new ranking.
  def change
    remove_index :feed_items, name: "idx_feed_items_timeline"
    add_index :feed_items, %i[user_id score sort_at],
              name: "idx_feed_items_timeline",
              order: { score: :desc, sort_at: :desc },
              where: "dismissed_at IS NULL AND acted_at IS NULL AND attention = false"
  end
end
