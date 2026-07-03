# frozen_string_literal: true

module Campbooks
  module Calendar
    # Renders an event's type icon inside chips/rows — the per-event visual
    # distinction now that chip color always comes from the owning calendar.
    # The glyph inherits currentColor, so contrast_on-driven chip text keeps it
    # readable on any calendar color. Untyped events (or types without an icon)
    # render nothing.
    module TypeIcon
      private

      def type_icon(event, css_class)
        icon = event.event_type&.icon.presence
        render Campbooks::Icon.new(icon, css_class: css_class) if icon
      end
    end
  end
end
