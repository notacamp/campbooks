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
        # Task-sourced reminders are excluded: the task has its own feed card, so
        # surfacing its deadline reminder too would double up.
        base = ::Reminder.accessible_to(user).includes(:source, :calendar_event)
                         .where(confidence: FEED_MIN_CONFIDENCE..)
                         .where(due_at: now.beginning_of_day..(now + HORIZON))
                         .where.not(source_type: "Task")
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

      # Urgency glides up as the date approaches: 92 when due (today), easing
      # through 75 at three days out and 60 at two weeks down to 30 at the far
      # horizon — no tier cliffs.
      def score_for(reminder)
        hours = (reminder.due_at - now) / 1.hour
        return 92 if hours <= 0 # due earlier today

        if hours <= 72
          ramp(hours, from: 0, to: 72, at_from: 92, at_to: 75)
        elsif hours <= 336
          ramp(hours, from: 72, to: 336, at_from: 75, at_to: 60)
        else
          ramp(hours, from: 336, to: HORIZON.in_hours, at_from: 60, at_to: 30)
        end
      end
    end
  end
end
