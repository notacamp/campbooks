module Feed
  module Sources
    # AI-detected reminders awaiting confirmation. Surfaces pending (and woken-up
    # snoozed) reminders the user can access, ranked by proximity. Confidence bar is
    # higher here than at extraction time — the noisy home feed shows only the
    # confident ones; the rest live on the /reminders page.
    class Reminder < Feed::Source
      FEED_MIN_CONFIDENCE = 0.6
      HORIZON = 60.days          # don't crowd the feed with far-future reminders
      ATTENTION_WITHIN = 3.days

      def self.key = "reminder"

      def candidates
        # `::Reminder` — disambiguate the model from this source class.
        # Floor at the start of today so past-due reminders (no longer actionable)
        # never surface; cap at the horizon so the feed isn't crowded by far-future ones.
        base = ::Reminder.accessible_to(user).includes(:source, :calendar_event)
                         .where(confidence: FEED_MIN_CONFIDENCE..)
                         .where(due_at: now.beginning_of_day..(now + HORIZON))
        ready = base.pending.or(base.snoozed.where(snoozed_until: ..now))

        ready.order(:due_at).map do |reminder|
          {
            subject: reminder,
            dedupe_key: "reminder:#{reminder.id}",
            sort_at: reminder.due_at,
            score: score_for(reminder),
            attention: reminder.due_at <= now + ATTENTION_WITHIN,
            data: { "reminder_type" => reminder.reminder_type, "due_at" => reminder.due_at.iso8601 }
          }
        end
      end

      def still_valid?(_item, reminder)
        return false if reminder.nil?
        reminder.pending? ||
          (reminder.snoozed? && reminder.snoozed_until.present? && reminder.snoozed_until <= now)
      end

      private

      def score_for(reminder)
        days = ((reminder.due_at - now) / 1.day).floor
        return 90 if days <= 0    # overdue
        return 80 if days <= 3    # imminent
        return 60 if days <= 14
        30
      end
    end
  end
end
