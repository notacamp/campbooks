# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Serializes a CalendarEvent for the first-party app API.
      # Mirrors Api::V1::CalendarEventSerializer but adds event_type metadata for
      # the calendar surface's event chips and detail sheet.
      # Pass detail: true for the full payload (attendees, rrule, description,
      # color, source_email_message_id, event_type).
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
            recurring: @event.recurring?,
            outbound_pending: @event.outbound_pending,
            event_type_id: @event.event_type_id,
            created_at: @event.created_at.iso8601
          }

          if @detail
            data[:description]             = @event.description
            data[:attendees]               = @event.attendees
            data[:rrule]                   = @event.rrule
            data[:source_email_message_id] = @event.source_email_message_id
            data[:event_type]              = event_type_data
          end

          data
        end

        private

        def event_type_data
          return nil unless @event.event_type
          { id: @event.event_type.id, name: @event.event_type.name, icon: @event.event_type.icon }
        end
      end
    end
  end
end
