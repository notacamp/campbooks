# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Serializes an EventType for the calendar surface.
      class EventTypeSerializer
        def initialize(event_type)
          @event_type = event_type
        end

        def as_json
          {
            id: @event_type.id,
            name: @event_type.name,
            icon: @event_type.icon,
            prompt: @event_type.prompt,
            created_at: @event_type.created_at.iso8601
          }
        end
      end
    end
  end
end
