module Campbooks
  module Calendar
    class DayGrid < Campbooks::Base
      include TimeGrid

      def initialize(date:, events:, reminders: [], snoozed_threads: [], scheduled_emails: [])
        @date = date
        @events = events.to_a
        @reminders = reminders.to_a
        @snoozed_threads = snoozed_threads.to_a
        @scheduled_emails = scheduled_emails.to_a
      end

      def view_template
        div(class: "overflow-hidden rounded-2xl border border-border/70 bg-card shadow-sm") do
          # Fills to the viewport bottom on lg+ and opens on the working hours.
          div(class: "relative max-h-[72vh] overflow-y-auto overscroll-contain lg:max-h-none",
              data: { controller: "calendar-fill-height",
                      "calendar-fill-height-scroll-to-hour-value": scroll_to_hour,
                      "calendar-fill-height-hour-px-value": HOUR_PX,
                      "calendar-fill-height-start-hour-value": START_HOUR,
                      "calendar-fill-height-min-value": 420 }) do
            render_all_day_row
            div(class: "relative", style: "height: #{grid_height}px") do
              render_hour_lines
              render_now_line if @date == Date.current
              div(
                class: "cursor-pointer",
                data: { controller: "calendar-dnd calendar-create",
                        "calendar-dnd-hour-px-value": HOUR_PX, "calendar-dnd-start-hour-value": START_HOUR,
                        "calendar-dnd-start-date-value": @date.iso8601, "calendar-dnd-days-value": 1,
                        "calendar-create-hour-px-value": HOUR_PX, "calendar-create-start-hour-value": START_HOUR,
                        "calendar-create-start-date-value": @date.iso8601, "calendar-create-days-value": 1 },
                style: "position:absolute; top:0; bottom:0; left:3.5rem; right:0.5rem;"
              ) do
                day_boxes(@events, @date).each { |box| render_event(box) }
              end
            end
          end
        end
      end

      private

      # Open on the working hours (or an hour before "now" when viewing today).
      def scroll_to_hour
        return 7 unless @date == Date.current
        [ [ Time.current.hour - 1, START_HOUR ].max, 18 ].min
      end

      def render_all_day_row
        all_day = @events.select(&:all_day)
        return if all_day.empty? && @reminders.empty? && @snoozed_threads.empty? && @scheduled_emails.empty?
        div(class: "sticky top-0 z-20 flex gap-2 border-b border-border/70 bg-card/80 px-3 py-2 backdrop-blur") do
          span(class: "w-11 shrink-0 pt-0.5 text-[11px] text-muted-foreground") { t("components.calendar.event_row.all_day") }
          div(class: "flex flex-1 flex-wrap gap-1") do
            all_day.each do |e|
              a(href: helpers.edit_calendar_event_path(e),
                data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(e) },
                class: "max-w-full truncate rounded px-2 py-0.5 text-xs font-medium",
                style: "background-color: #{e.display_color}; color: #{contrast_on(e.display_color)}") { e.title.presence || t("components.calendar.event_row.untitled") }
            end
            @reminders.each { |reminder| render Campbooks::Calendar::ReminderChip.new(reminder: reminder) }
            @snoozed_threads.each { |thread| render Campbooks::Calendar::SnoozedChip.new(thread: thread) }
            @scheduled_emails.each { |email| render Campbooks::Calendar::ScheduledEmailChip.new(scheduled_email: email) }
          end
        end
      end

      def render_hour_lines
        hours.each do |h|
          div(class: "absolute left-0 right-0 border-t border-border/50", style: "top: #{hour_top(h)}px") do
            span(class: "absolute -top-2 left-2 bg-card pr-1 text-[10px] tabular-nums text-muted-foreground") { hour_label(@date, h) }
          end
        end
      end

      def render_now_line
        minutes = (Time.current.hour - START_HOUR) * 60 + Time.current.min
        top = (minutes / 60.0) * HOUR_PX
        div(class: "pointer-events-none z-10 border-t-2 border-red-500", style: "position:absolute; left:3rem; right:0; top:#{top.round}px") do
          span(class: "h-2.5 w-2.5 rounded-full bg-red-500", style: "position:absolute; left:-5px; top:-6px")
        end
      end

      def render_event(box)
        e = box[:event]
        color = e.display_color
        drag = e.calendar.is_writable
        a(href: helpers.edit_calendar_event_path(e),
          data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(e) }.merge(drag ? { "calendar-dnd-target": "event", "event-id": e.id } : {}),
          class: class_names("absolute overflow-hidden rounded-lg px-2 py-1 text-xs leading-tight shadow-sm ring-1 ring-black/5", ("cursor-grab" if drag)),
          style: "top:#{box[:top]}px; height:#{box[:height]}px; left:#{box[:left]}%; width:#{box[:width]}%; background-color:#{color}; color:#{contrast_on(color)}") do
          span(class: "block truncate font-semibold") { e.title.presence || t("components.calendar.event_row.untitled") }
          span(class: "block truncate opacity-90") { l(e.start_at, format: :clock) }
        end
      end
    end
  end
end
