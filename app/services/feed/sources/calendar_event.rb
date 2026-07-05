module Feed
  module Sources
    # Imminent meetings: timed events on the user's readable calendars starting
    # within the next few hours, surfaced with a Join/Open action. Promoted to the
    # attention cluster once they're close (within ATTENTION_WITHIN).
    class CalendarEvent < Feed::Source
      WINDOW = 8.hours
      ATTENTION_WITHIN = 45.minutes

      def self.key = "calendar_event"

      def candidates
        # `::CalendarEvent` — disambiguate the model from this source class.
        ::CalendarEvent.accessible_to(user).visible
          .where(all_day: false)
          .where(start_at: now..(now + WINDOW))
          .order(:start_at)
          .includes(:event_type, calendar: :calendar_account)
          .map do |event|
            {
              subject: event,
              dedupe_key: "calendar_event:#{event.id}",
              sort_at: event.start_at,
              score: score_for(event),
              attention: event.start_at <= now + ATTENTION_WITHIN,
              data: { starts_at: event.start_at.iso8601 }
            }
          end
      end

      def still_valid?(item, event)
        return false if event.nil? || event.cancelled?
        event.end_at.nil? || event.end_at > now # not yet ended
      end

      private

      # Urgency climbs as the start approaches: 40 at the window's edge, 100 at
      # start time (~94 when it crosses into the attention cluster).
      def score_for(event)
        minutes_until = (event.start_at - now) / 60.0
        ramp(minutes_until, from: WINDOW.in_minutes, to: 0, at_from: 40, at_to: 100)
      end
    end
  end
end
