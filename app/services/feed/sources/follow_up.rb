module Feed
  module Sources
    # Conversations the user replied to and is still waiting to hear back on — the
    # proactive subset of Emails::AwaitingReply#due. Visibility is pure data (the
    # user holds the last word); the AI only enriches the timing/wording, so this
    # fires even when no AI provider is configured. The home-feed counterpart of
    # Skim's Follow-ups ring.
    #
    # Distinct from ReplyReminder, which nudges mail the user HASN'T replied to (the
    # ball in their own court). Here the ball is in the other party's court, so the
    # card offers an AI-drafted nudge rather than a "you haven't replied" reminder.
    # Registered before ReplyReminder/EmailAction so a follow-up thread is framed as
    # a follow-up, never re-surfaced as a generic action.
    class FollowUp < Feed::Source
      SCORE = 80

      def self.key = "follow_up"

      def candidates
        threads = Emails::AwaitingReply.new(user, now: now).due
        collapse_by_thread(threads.filter_map { |thread| candidate_for(thread) })
      end

      def still_valid?(_item, message)
        return false if message.nil?

        thread = message.email_thread
        return false unless thread

        thread.holds_last_word? && thread.follow_up_dismissed_at.nil? && in_inbox?(message)
      end

      private

      def candidate_for(thread)
        message = representative(thread)
        return nil unless message && admitted_message?(message) && in_inbox?(message)

        {
          subject: message,
          dedupe_key: "follow_up:#{thread.id}",
          sort_at: thread.follow_up_at || now,
          score: SCORE,
          attention: true,
          data: {
            "reason" => thread.follow_up_reason.to_s,
            "since" => thread.last_outbound_at&.iso8601,
            "age_days" => age_days(thread.last_outbound_at)
          }
        }
      end

      # The other party's most recent message — who we're nudging and what the card
      # shows. nil for a cold outbound with no inbound message (nothing to surface).
      def representative(thread)
        addr = thread.email_account&.email_address.to_s.downcase
        return nil if addr.blank?

        thread.email_messages
              .sort_by { |m| m.received_at || Time.at(0) }
              .reverse
              .find { |m| !m.from_address.to_s.downcase.include?(addr) }
      end

      def age_days(time)
        return 0 unless time

        ((now - time) / 1.day).floor
      end
    end
  end
end
