# frozen_string_literal: true

# Data-only cleanup: resolve pending (unactioned, undismissed) tag_suggestion
# feed items that were created before this release. Those items were generated
# under the old ask-style UX ("File it / Not now") and carry no data["applied"]
# flag, so the new card would render them as legacy. Expiring them here means the
# next feed generation produces fresh notice-style cards with the tag already
# applied, giving every user a clean slate on upgrade.
#
# Only targets feed items that are still active (no acted_at, dismissed_at, or
# expired_at) and lack the applied flag — acted/dismissed rows are kept as-is
# so history stats remain intact.
#
# Idempotent and defensive — a bad row is logged and skipped.
class ExpirePendingTagSuggestionFeedItems < ActiveRecord::Migration[8.1]
  def up
    FeedItem.reset_column_information
    now = Time.current

    count = FeedItem
      .where(kind: "tag_suggestion")
      .where(acted_at: nil, dismissed_at: nil, expired_at: nil)
      .where("(data->>'applied') IS DISTINCT FROM 'true'")
      .update_all(expired_at: now, updated_at: now)

    say "#{count} pending legacy tag_suggestion item(s) expired"
  rescue StandardError => e
    say "tag_suggestion expiry skipped: #{e.class}: #{e.message}"
  end

  def down
    # Data-only; nothing meaningful to undo.
  end
end
