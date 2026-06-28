# frozen_string_literal: true

module Api
  module V1
    # Serializes a CalendarEvent for the public API. List responses omit the
    # heavy detail fields; pass detail: true (show / create / update / rsvp) to
    # include description, attendees, and rrule.
    class CalendarEventSerializer
      def initialize(event, detail: false)
        @event = event
        @detail = detail
      end

      def as_json
        data = {
          id: @event.id,
          title: @event.title,
          location: @event.location,
          start_at: @event.start_at&.iso8601,
          end_at: @event.end_at&.iso8601,
          all_day: @event.all_day,
          status: @event.status,
          rsvp_status: @event.rsvp_status,
          color: @event.display_color,
          calendar_id: @event.calendar_id,
          conference_url: @event.conference_url,
          html_link: @event.html_link,
          is_organizer: @event.is_organizer,
          recurring: @event.rrule.present?,
          source_email_message_id: @event.source_email_message_id,
          created_at: @event.created_at.iso8601
        }

        if @detail
          data[:description] = @event.description
          data[:attendees]   = @event.attendees
          data[:rrule]       = @event.rrule
        end

        data
      end
    end
  end
end
