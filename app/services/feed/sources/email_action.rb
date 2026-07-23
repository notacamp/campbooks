module Feed
  module Sources
    # The broad catch-all: emails Scout flagged with a live action prompt (the
    # current home-feed content). Rendered as the bordered "hero" card with
    # Scout's read + one-tap suggested actions.
    #
    # Aged "needs a reply" mail is claimed first by ReplyReminder and filing-only
    # mail by TagSuggestion (see Feed::Source.all ordering + Generator dedup), so
    # what's left here is fresh / generic actionable mail.
    class EmailAction < Feed::Source
      def self.key = "email_action"

      def candidates
        collapse_by_thread(
          base_scope.reorder(:id).find_each.map do |m|
            {
              subject: m,
              dedupe_key: "email_action:#{m.id}",
              sort_at: m.received_at || m.created_at,
              score: score_for(m),
              attention: attention?(m),
              data: {}
            }
          end
        )
      end

      def still_valid?(_item, m)
        return false if m.nil?
        return false if m.email_thread&.holds_last_word? # already answered — FollowUp owns it now
        m.ai_action_prompt.present? && !m.ai_todo_dismissed? && m.skimmed_at.nil? && in_inbox?(m)
      end

      private

      # Reuses the partial index idx_email_messages_ai_todos. Only the columns
      # needed to rank/route are loaded — never `body`, so generation stays light
      # even reaching back across the whole flagged set.
      def base_scope
        in_inbox(admitted(
          EmailMessage.accessible_to(user).with_ai_todos.where(skimmed_at: nil)
            .not_answered_by_owner
            .select(:id, :received_at, :created_at, :ai_priority, :pinned_at, :read,
                    :category, :email_account_id, :email_thread_id, :contact_id,
                    :subject, :from_address) # the Generator's conversation claim
        ))
      end

      # Intrinsic urgency (0–100 band; Feed::Ranking layers relevance + decay on
      # top): generic actionable mail sits mid-feed, Scout's high-priority read
      # and the user's own pin push it toward the attention tier.
      def score_for(m)
        s = 45
        s += 40 if m.ai_priority == "high"
        s += 30 if m.pinned_at.present?
        s += 5 unless m.read?
        s
      end

      def attention?(m)
        m.ai_priority == "high" || m.pinned_at.present?
      end
    end
  end
end
