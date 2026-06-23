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
          .includes(calendar: :calendar_account)
          .map do |event|
            soon = event.start_at <= now + ATTENTION_WITHIN
            {
              subject: event,
              dedupe_key: "calendar_event:#{event.id}",
              sort_at: event.start_at,
              score: soon ? 95 : 40,
              attention: soon,
              data: { starts_at: event.start_at.iso8601 }
            }
          end
      end

      def still_valid?(item, event)
        return false if event.nil? || event.cancelled?
        event.end_at.nil? || event.end_at > now # not yet ended
      end
    end
  end
end
