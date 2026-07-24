module Feed
  module Sources
    # Quiet nudges: Scout-flagged mail you were expected to reply to but have sat
    # on (aged > 3 days), plus threads whose snooze has come due. Distinct from
    # EmailAction in tone (a gentle "you haven't replied", not "here's a draft")
    # and rendered as a borderless nudge.
    class ReplyReminder < Feed::Source
      AGED_DAYS = 3
      OVERDUE_DAYS = 7

      def self.key = "reply_reminder"

      def candidates
        # A message can be both aged-unreplied and in a due-snooze thread; keep
        # the higher-scored (snooze-due) framing for it, then one card per thread.
        merged = (no_reply_candidates + snooze_due_candidates)
                 .group_by { |c| c[:dedupe_key] }
                 .map { |_key, dupes| dupes.max_by { |c| c[:score] } }
        collapse_by_thread(merged)
      end

      def still_valid?(item, m)
        return false if m.nil? || m.skimmed_at.present?
        return false if replied?(m)

        if item.data["reason"] == "snooze_due"
          # Snoozed mail lives in a Snoozed folder, not the inbox — never gate it.
          true
        else
          m.ai_action_prompt.present? && !m.ai_todo_dismissed? && in_inbox?(m)
        end
      end

      private

      def no_reply_candidates
        aged_scope.reorder(:id).find_each.filter_map do |m|
          next if replied?(m) # already answered — don't nag

          age = age_days(m.received_at)
          {
            subject: m,
            dedupe_key: "reply_reminder:#{m.id}",
            sort_at: m.received_at,
            # The nudge firms up as the silence stretches: 35 when it first
            # qualifies, 75 (near the attention tier) once a week has passed.
            # Feed::Ranking's recency decay then fades it — a nag ignored for
            # months goes quiet instead of climbing.
            score: ramp((now - m.received_at) / 1.day,
                        from: AGED_DAYS, to: OVERDUE_DAYS, at_from: 35, at_to: 75),
            attention: age >= OVERDUE_DAYS || m.ai_priority == "high",
            data: { reason: "no_reply", since: m.received_at.iso8601, age_days: age }
          }
        end
      end

      def snooze_due_candidates
        EmailThread.expired_snoozes
          .where(email_account: user.readable_email_accounts)
          .find_each.filter_map do |thread|
            m = thread.email_messages.accessible_to(user).order(received_at: :desc).first
            next unless m
            next unless admitted_message?(m) # respect block / whitelist gating
            next if replied?(m) # already answered after snoozing
            {
              subject: m,
              dedupe_key: "reply_reminder:#{m.id}",
              sort_at: thread.snoozed_until || now,
              score: 90,
              attention: true,
              data: { reason: "snooze_due" }
            }
          end
      end

      # Aged, still-open mail that expects a response (Scout suggested a reply, or
      # marked it high priority).
      def aged_scope
        expects_reply = EmailMessage.where("ai_suggested_actions @> ?", draft_reply_json)
                                    .or(EmailMessage.where(ai_priority: :high))

        in_inbox(admitted(
          EmailMessage.accessible_to(user).with_ai_todos
            .where(skimmed_at: nil)
            .where("received_at < ?", now - AGED_DAYS.days)
            .merge(expects_reply)
            .select(:id, :received_at, :ai_priority, :category, :email_account_id,
                    :email_thread_id, :contact_id,
                    :subject, :from_address) # the Generator's conversation claim
        ))
      end

      def age_days(t) = ((now - t) / 1.day).floor

      def draft_reply_json = [ { tool: "draft_reply" } ].to_json

      # True once a later message from the mailbox owner exists in the thread —
      # i.e. someone already replied, so stop nagging. One small query per item
      # (the reminder set is small); used on read + during reconcile.
      def replied?(m)
        thread = m.email_thread
        return false unless thread
        addr = m.email_account&.email_address.to_s.downcase
        return false if addr.blank?

        thread.email_messages
              .where("received_at > ?", m.received_at)
              .where("LOWER(from_address) = ?", addr)
              .exists?
      end
    end
  end
end
