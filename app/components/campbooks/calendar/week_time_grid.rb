module Campbooks
  module Calendar
    # Desktop 7-day time grid: a shared hour axis on the left and seven day columns
    # with positioned events. Shares the hour scale + overlap packing with DayGrid
    # via TimeGrid; each day's boxes (0-100% of one day) are scaled into that day's
    # 1/7 slice of the track. Paired with the responsive WeekGrid column-list on
    # mobile (the calendar view shows this only at md+).
    class WeekTimeGrid < Campbooks::Base
      include TimeGrid

      DAY_W = 100.0 / 7

      def initialize(date:, events:, reminders: [])
        @date = date
        @events = events.to_a
        @reminders = reminders.to_a
      end

      def view_template
        div(class: "overflow-hidden rounded-xl border border-border bg-card") do
          render_header
          render_all_day_row
          div(class: "flex") do
            render_hour_axis
            div(class: "relative flex-1 cursor-pointer",
                data: { controller: "calendar-dnd calendar-create",
                        "calendar-dnd-hour-px-value": HOUR_PX, "calendar-dnd-start-hour-value": START_HOUR,
                        "calendar-dnd-start-date-value": days.first.iso8601, "calendar-dnd-days-value": 7,
                        "calendar-create-hour-px-value": HOUR_PX, "calendar-create-start-hour-value": START_HOUR,
                        "calendar-create-start-date-value": days.first.iso8601, "calendar-create-days-value": 7 },
                style: "height: #{grid_height}px") do
              render_day_separators
              render_hour_lines
              render_now_line if days.include?(Date.current)
              days.each_with_index do |day, i|
                day_boxes(events_for(day), day).each { |box| render_event(box, i) }
              end
            end
          end
        end
      end

      private

      def days = (@date.beginning_of_week..@date.end_of_week).to_a

      def by_day
        @by_day ||= @events.group_by { |e| e.start_at&.to_date }
      end

      def events_for(day) = by_day[day] || []

      def reminders_by_day
        @reminders_by_day ||= @reminders.group_by { |r| r.due_at.to_date }
      end

      def reminders_for(day) = reminders_by_day[day] || []

      def render_header
        div(class: "flex border-b border-border") do
          div(class: "w-14 shrink-0")
          days.each do |day|
            today = day == Date.current
            div(class: "flex-1 px-1 py-2 text-center") do
              div(class: class_names("text-[11px] font-semibold uppercase", today ? "text-primary" : "text-gray-400")) { t("date.abbr_day_names")[day.wday] }
              div(class: class_names("text-sm", today ? "font-semibold text-primary" : "text-gray-600")) { day.day.to_s }
            end
          end
        end
      end

      def render_all_day_row
        return unless days.any? { |d| events_for(d).any?(&:all_day) } || @reminders.any?
        div(class: "flex border-b border-border") do
          div(class: "w-14 shrink-0 px-2 py-1 text-[10px] text-muted-foreground") { t("components.calendar.event_row.all_day") }
          days.each do |day|
            div(class: "min-w-0 flex-1 space-y-0.5 px-1 py-1") do
              events_for(day).select(&:all_day).each do |e|
                a(href: helpers.edit_calendar_event_path(e),
                  data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(e) },
                  class: "block truncate rounded px-1 text-[10px]",
                  style: "background-color: #{e.display_color}; color: #{contrast_on(e.display_color)}") { e.title.presence || t("components.calendar.event_row.untitled") }
              end
              reminders_for(day).each { |reminder| render Campbooks::Calendar::ReminderChip.new(reminder: reminder) }
            end
          end
        end
      end

      def render_hour_axis
        div(class: "relative w-14 shrink-0", style: "height: #{grid_height}px") do
          hours.each do |h|
            span(class: "absolute left-2 text-[10px] text-muted-foreground", style: "top: #{hour_top(h) - 6}px") { hour_label(@date, h) }
          end
        end
      end

      def render_day_separators
        (1...days.size).each do |i|
          div(class: "absolute bottom-0 top-0 border-l border-hair", style: "left: #{(DAY_W * i).round(3)}%")
        end
      end

      def render_hour_lines
        hours.each do |h|
          next if h == START_HOUR
          div(class: "absolute left-0 right-0 border-t border-hair", style: "top: #{hour_top(h)}px")
        end
      end

      def render_now_line
        minutes = (Time.current.hour - START_HOUR) * 60 + Time.current.min
        top = ((minutes / 60.0) * HOUR_PX).round
        div(class: "pointer-events-none absolute left-0 right-0 z-10 border-t border-red-500/70", style: "top: #{top}px")
      end

      def render_event(box, day_index)
        e = box[:event]
        color = e.display_color
        drag = e.calendar.is_writable
        left = (day_index * DAY_W) + (box[:left] / 100.0 * DAY_W)
        width = [ (box[:width] / 100.0 * DAY_W) - 0.3, 1 ].max
        a(href: helpers.edit_calendar_event_path(e),
          data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(e) }.merge(drag ? { "calendar-dnd-target": "event", "event-id": e.id } : {}),
          class: class_names("absolute overflow-hidden rounded px-1 py-0.5 text-[10px] leading-tight ring-1 ring-black/5", ("cursor-grab" if drag)),
          style: "top:#{box[:top]}px; height:#{box[:height]}px; left:#{left.round(3)}%; width:#{width.round(3)}%; background-color:#{color}; color:#{contrast_on(color)}") do
          span(class: "block truncate font-medium") { e.title.presence || t("components.calendar.event_row.untitled") }
        end
      end
    end
  end
end
