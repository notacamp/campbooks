# frozen_string_literal: true

module Feed
  module Sources
    # Actionable notices on the feed: the user's "Needs you" notifications
    # (action_required, still active) surfaced as decision cards, so a disconnected
    # mailbox or a document that failed to process lands in the deck next to the
    # mail that needs a reply — one queue of decisions, not a separate bell to check.
    #
    # A notice is always attention (it's action_required by definition) and scores
    # near the top of the intrinsic band so it pins above ambient mail; recency decay
    # (Feed::Ranking) still lets a stale, never-addressed notice drift down over time.
    # Registered FIRST in Feed::Source.all — a Notification subject never overlaps
    # another source's records, but keeping it first makes that intent explicit.
    class Notice < Feed::Source
      # High on the 0–100 intrinsic band: an action_required notice should sit in
      # the pinned attention cluster (ATTENTION_FLOOR is 55) while it's fresh.
      SCORE = 95

      def self.key = "notice"

      def candidates
        user.notifications.needs_action.order(created_at: :desc).map do |notification|
          {
            subject:    notification,
            dedupe_key: "notice:#{notification.id}",
            sort_at:    notification.created_at,
            score:      SCORE,
            attention:  true,
            data:       {}
          }
        end
      end

      # Still worth showing while the notification still needs action — not archived
      # (the user pressed Done), not auto-resolved (the underlying state cleared).
      def still_valid?(_item, subject)
        subject.present? && subject.active? && subject.priority_action_required?
      end
    end
  end
end
