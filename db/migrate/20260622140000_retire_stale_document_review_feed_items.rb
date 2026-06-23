# frozen_string_literal: true

# Retire feed_items left behind by the removed `document_review` source (dropped
# 2026-06-21 when documents moved to the home "Documents" ring). No card maps to
# the kind anymore, so they never render — but they were still `active`, padding
# the "N things want you" count and the active-item total.
#
# Soft-retire (set acted_at), matching how Feed::Generator reconciles items that
# a source no longer claims, rather than deleting derived rows. A refresh
# re-materializes anything that's genuinely live.
class RetireStaleDocumentReviewFeedItems < ActiveRecord::Migration[8.1]
  def up
    now = connection.quote(Time.current)
    execute(<<~SQL.squish)
      UPDATE feed_items
      SET acted_at = #{now}, updated_at = #{now}
      WHERE kind = 'document_review'
        AND acted_at IS NULL
        AND dismissed_at IS NULL
    SQL
  end

  def down
    # Irreversible: once retired we can't distinguish these from genuinely-acted
    # rows. Harmless to leave retired — they have no card to render.
  end
end
