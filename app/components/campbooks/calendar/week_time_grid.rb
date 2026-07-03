module Campbooks
  module Calendar
    class WeekTimeGrid < Campbooks::Base
      include TimeGrid
      include TypeIcon
      DAY_W = 100.0 / 7

      def initialize(date:, events:, reminders: [], snoozed_threads: [], scheduled_emails: [])
        @date = date
        @events = events.to_a
        @reminders = reminders.to_a
        @snoozed_threads = snoozed_threads.to_a
        @scheduled_emails = scheduled_emails.to_a
      end

      def view_template
        div(class: "overflow-hidden rounded-2xl border border-border/70 bg-card shadow-sm") do
          # The scroll viewport fills to the viewport bottom on lg+ and opens on the
          # working hours (calendar-fill-height), so you land on the busy part of the
          # day instead of an empty pre-dawn grid. Below lg it's a capped scroll pane.
          div(class: "relative max-h-[72vh] overflow-y-auto overscroll-contain lg:max-h-none",
              data: { controller: "calendar-fill-height",
                      "calendar-fill-height-scroll-to-hour-value": scroll_to_hour,
                      "calendar-fill-height-hour-px-value": HOUR_PX,
                      "calendar-fill-height-start-hour-value": START_HOUR,
                      "calendar-fill-height-min-value": 420 }) do
            # Sticky frosted bar: day names + all-day events pin to the top while the
            # hours scroll underneath — the calendar's real "glass" surface.
            div(class: "sticky top-0 z-20 border-b border-border/70 bg-card/80 backdrop-blur") do
              render_header
              render_all_day_row
            end
            div(class: "flex") do
              render_hour_axis
              div(class: "relative flex-1 cursor-pointer",
                  data: { controller: "calendar-dnd calendar-create",
                          "calendar-dnd-hour-px-value": HOUR_PX, "calendar-dnd-start-hour-value": START_HOUR,
                          "calendar-dnd-start-date-value": days.first.iso8601, "calendar-dnd-days-value": 7,
                          "calendar-create-hour-px-value": HOUR_PX, "calendar-create-start-hour-value": START_HOUR,
                          "calendar-create-start-date-value": days.first.iso8601, "calendar-create-days-value": 7 },
                  style: "height: #{grid_height}px") do
                render_today_column
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
      end

      private

      def days = (@date.beginning_of_week..@date.end_of_week).to_a
      def by_day = @by_day ||= @events.group_by { |e| e.start_at&.to_date }
      def events_for(day) = by_day[day] || []
      def reminders_by_day = @reminders_by_day ||= @reminders.group_by { |r| r.due_at.to_date }
      def reminders_for(day) = reminders_by_day[day] || []
      def snoozed_by_day = @snoozed_by_day ||= @snoozed_threads.group_by { |t| t.snoozed_until.to_date }
      def snoozed_for(day) = snoozed_by_day[day] || []
      def scheduled_by_day = @scheduled_by_day ||= @scheduled_emails.group_by { |s| (s.next_occurrence_at || s.scheduled_at).to_date }
      def scheduled_for(day) = scheduled_by_day[day] || []

      # Open on the working hours (or an hour before "now" when today is in view)
      # rather than midnight, so the busy part of the day is what you see first.
      def scroll_to_hour
        return 7 unless days.include?(Date.current)
        [ [ Time.current.hour - 1, START_HOUR ].max, 18 ].min
      end

      def render_header
        div(class: "flex") do
          div(class: "w-14 shrink-0")
          days.each do |day|
            today = day == Date.current
            div(class: "flex-1 px-1 py-2 text-center") do
              div(class: class_names("text-[11px] font-semibold uppercase tracking-wide", today ? "text-foreground" : "text-muted-foreground")) { t("date.abbr_day_names")[day.wday] }
              div(class: "mt-1 flex justify-center") do
                span(class: class_names("flex h-7 w-7 items-center justify-center rounded-full text-sm transition-colors", today ? "bg-primary font-semibold text-primary-foreground" : "text-gray-700")) { day.day.to_s }
              end
            end
          end
        end
      end

      def render_all_day_row
        return unless days.any? { |d| events_for(d).any?(&:all_day) } || @reminders.any? || @snoozed_threads.any? || @scheduled_emails.any?
        div(class: "flex border-t border-border/60") do
          div(class: "w-14 shrink-0 px-2 py-1 text-[10px] text-muted-foreground") { t("components.calendar.event_row.all_day") }
          days.each do |day|
            div(class: "min-w-0 flex-1 space-y-0.5 px-1 py-1") do
              events_for(day).select(&:all_day).each do |e|
                a(href: helpers.edit_calendar_event_path(e),
                  data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(e) },
                  class: "block rounded px-1 text-[10px] font-medium",
                  style: "background-color: #{e.display_color}; color: #{contrast_on(e.display_color)}") do
                  span(class: "flex min-w-0 items-center gap-0.5") do
                    type_icon(e, "h-2.5 w-2.5 flex-shrink-0")
                    span(class: "min-w-0 truncate") { e.title.presence || t("components.calendar.event_row.untitled") }
                  end
                end
              end
              reminders_for(day).each { |reminder| render Campbooks::Calendar::ReminderChip.new(reminder: reminder) }
              snoozed_for(day).each { |thread| render Campbooks::Calendar::SnoozedChip.new(thread: thread) }
              scheduled_for(day).each { |email| render Campbooks::Calendar::ScheduledEmailChip.new(scheduled_email: email) }
            end
          end
        end
      end

      def render_hour_axis
        div(class: "relative w-14 shrink-0", style: "height: #{grid_height}px") do
          hours.each do |h|
            next if h == START_HOUR
            span(class: "absolute right-2 text-[10px] tabular-nums text-muted-foreground", style: "top: #{hour_top(h) - 6}px") { hour_label(@date, h) }
          end
        end
      end

      # A whisper-faint wash behind today's column so the eye finds "today" in the
      # week at a glance (ink-neutral, never Ember).
      def render_today_column
        idx = days.index(Date.current)
        return unless idx
        div(class: "pointer-events-none absolute bottom-0 top-0 bg-muted/40",
            style: "left: #{(DAY_W * idx).round(3)}%; width: #{DAY_W.round(3)}%")
      end

      def render_day_separators
        (1...days.size).each do |i|
          div(class: "absolute bottom-0 top-0 border-l border-border/50", style: "left: #{(DAY_W * i).round(3)}%")
        end
      end

      def render_hour_lines
        hours.each do |h|
          next if h == START_HOUR
          div(class: "absolute left-0 right-0 border-t border-border/50", style: "top: #{hour_top(h)}px")
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
          class: class_names("absolute overflow-hidden rounded-md px-1.5 py-0.5 text-[11px] leading-tight shadow-sm ring-1 ring-black/5", ("cursor-grab" if drag)),
          style: "top:#{box[:top]}px; height:#{box[:height]}px; left:#{left.round(3)}%; width:#{width.round(3)}%; background-color:#{color}; color:#{contrast_on(color)}") do
          span(class: "flex min-w-0 items-center gap-0.5 font-semibold") do
            type_icon(e, "h-2.5 w-2.5 flex-shrink-0")
            span(class: "min-w-0 truncate") { e.title.presence || t("components.calendar.event_row.untitled") }
          end
          span(class: "block truncate opacity-90") { l(e.start_at, format: :clock) } if box[:height] >= 38 && e.start_at
        end
      end
    end
  end
end
