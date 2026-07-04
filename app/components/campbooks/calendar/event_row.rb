module Campbooks
  module Calendar
    # A single event line in the agenda view: time · color bar · title/location.
    # Links to the event's edit page (the view+edit surface for v1).
    class EventRow < Campbooks::Base
      include TimeUntil
      include TypeIcon

      def initialize(event:)
        @event = event
      end

      def view_template
        a(
          href: helpers.edit_calendar_event_path(@event),
          data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(@event) },
          class: "-mx-3 flex items-center gap-3 rounded-xl px-3 py-2.5 transition-colors hover:bg-muted/50"
        ) do
          span(class: "w-16 shrink-0 text-xs text-muted-foreground tabular-nums") { time_label }
          span(class: "h-2 w-2 shrink-0 rounded-full", style: "background-color: #{@event.display_color}")
          div(class: "min-w-0 flex-1") do
            span(class: "flex min-w-0 items-center gap-1.5 text-sm text-foreground") do
              type_icon(@event, "h-3 w-3 flex-shrink-0 text-muted-foreground")
              span(class: "min-w-0 truncate") { @event.title.presence || t(".untitled") }
            end
            if (sub = subtitle)
              span(class: "block text-xs text-gray-400 truncate") { sub }
            end
          end
          countdown
          render Campbooks::RecurrenceIcon.new(css: "w-3.5 h-3.5 text-gray-300 shrink-0") if @event.recurring?
        end
      end

      private

      # A muted "in X days" / "in 3 h" countdown, right-aligned; accented when
      # the event is now / within the hour / today (see TimeUntil#imminent).
      def countdown
        label = time_until_label(@event.start_at, all_day: @event.all_day)
        return unless label

        span(class: class_names(
          "shrink-0 whitespace-nowrap text-xs tabular-nums",
          label.imminent ? "font-medium text-accent-700" : "text-muted-foreground"
        )) { label.text }
      end

      def time_label
        @event.all_day ? t(".all_day") : l(@event.start_at, format: :clock)
      end

      def subtitle
        @event.location.presence || (@event.conference_url.present? ? t(".video_call") : nil)
      end
    end
  end
end
