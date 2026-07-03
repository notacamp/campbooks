module Campbooks
  module Calendar
    # A single event line in the agenda view: time · color bar · title/location.
    # Links to the event's edit page (the view+edit surface for v1).
    class EventRow < Campbooks::Base
      include TimeUntil

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
            span(class: "block text-sm text-foreground truncate") { @event.title.presence || t(".untitled") }
            if (sub = subtitle)
              span(class: "block text-xs text-gray-400 truncate") { sub }
            end
          end
          countdown
          raw(safe(recurring_icon)) if @event.recurring?
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

      def recurring_icon
        '<svg class="w-3.5 h-3.5 text-gray-300 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M17 2l4 4-4 4"/><path d="M3 11v-1a4 4 0 014-4h14"/><path d="M7 22l-4-4 4-4"/><path d="M21 13v1a4 4 0 01-4 4H3"/></svg>'
      end
    end
  end
end
