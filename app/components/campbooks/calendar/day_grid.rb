module Campbooks
  module Calendar
    class DayGrid < Campbooks::Base
      include TimeGrid
      include TypeIcon

      def initialize(date:, events:, reminders: [], snoozed_threads: [], scheduled_emails: [])
        @date = date
        @events = events.to_a
        @reminders = reminders.to_a
        @snoozed_threads = snoozed_threads.to_a
        @scheduled_emails = scheduled_emails.to_a
      end

      def view_template
        div(class: "overflow-hidden rounded-xl border border-border bg-card") do
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

      private

      def render_all_day_row
        all_day = @events.select(&:all_day)
        return if all_day.empty? && @reminders.empty? && @snoozed_threads.empty? && @scheduled_emails.empty?
        div(class: "flex gap-2 border-b border-border px-3 py-2") do
          span(class: "w-11 shrink-0 pt-0.5 text-[11px] text-muted-foreground") { t("components.calendar.event_row.all_day") }
          div(class: "flex flex-1 flex-wrap gap-1") do
            all_day.each do |e|
              a(href: helpers.edit_calendar_event_path(e),
                data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(e) },
                class: "max-w-full rounded px-2 py-0.5 text-xs",
                style: "background-color: #{e.display_color}; color: #{contrast_on(e.display_color)}") do
                span(class: "flex min-w-0 items-center gap-1") do
                  type_icon(e, "h-3 w-3 flex-shrink-0")
                  span(class: "min-w-0 truncate") { e.title.presence || t("components.calendar.event_row.untitled") }
                end
              end
            end
            @reminders.each { |reminder| render Campbooks::Calendar::ReminderChip.new(reminder: reminder) }
            @snoozed_threads.each { |thread| render Campbooks::Calendar::SnoozedChip.new(thread: thread) }
            @scheduled_emails.each { |email| render Campbooks::Calendar::ScheduledEmailChip.new(scheduled_email: email) }
          end
        end
      end

      def render_hour_lines
        hours.each do |h|
          div(class: "absolute left-0 right-0 border-t border-hair", style: "top: #{hour_top(h)}px") do
            span(class: "absolute left-2 -top-2 bg-card pr-1 text-[10px] text-muted-foreground") { hour_label(@date, h) }
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
          class: class_names("absolute overflow-hidden rounded-lg px-2 py-1 text-xs leading-tight ring-1 ring-black/5", ("cursor-grab" if drag)),
          style: "top:#{box[:top]}px; height:#{box[:height]}px; left:#{box[:left]}%; width:#{box[:width]}%; background-color:#{color}; color:#{contrast_on(color)}") do
          span(class: "flex min-w-0 items-center gap-1 font-medium") do
            type_icon(e, "h-3 w-3 flex-shrink-0")
            span(class: "min-w-0 truncate") { e.title.presence || t("components.calendar.event_row.untitled") }
          end
          span(class: "block truncate opacity-90") { l(e.start_at, format: :clock) }
        end
      end
    end
  end
end
