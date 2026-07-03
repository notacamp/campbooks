module Campbooks
  module Calendar
    class MonthGrid < Campbooks::Base
      include TypeIcon
      MAX_CHIPS = 3

      def initialize(date:, events:, reminders: [], snoozed_threads: [], scheduled_emails: [])
        @date = date
        @events = events.to_a
        @reminders = reminders.to_a
        @snoozed_threads = snoozed_threads.to_a
        @scheduled_emails = scheduled_emails.to_a
      end

      def view_template
        div(class: "rounded-xl border border-border overflow-hidden bg-card",
            data: { controller: "calendar-create calendar-month-dnd", "calendar-create-mode-value": "month" }) do
          div(class: "grid grid-cols-7 border-b border-border") do
            weekday_names.each do |name|
              div(class: "px-1 py-2 text-[11px] font-semibold uppercase tracking-wide text-gray-400 text-center truncate") { name }
            end
          end
          div(class: "grid grid-cols-7") do
            days.each { |day| render_day(day) }
          end
        end
      end

      private

      def days
        (@date.beginning_of_month.beginning_of_week..@date.end_of_month.end_of_week).to_a
      end

      def by_day
        @by_day ||= @events.group_by { |e| e.start_at.to_date }
      end

      def reminders_by_day
        @reminders_by_day ||= @reminders.group_by { |r| r.due_at.to_date }
      end

      def snoozed_by_day
        @snoozed_by_day ||= @snoozed_threads.group_by { |t| t.snoozed_until.to_date }
      end

      def scheduled_by_day
        @scheduled_by_day ||= @scheduled_emails.group_by { |s| (s.next_occurrence_at || s.scheduled_at).to_date }
      end

      def weekday_names
        names = t("date.abbr_day_names")
        (1..7).map { |i| names[i % 7] }
      end

      def render_day(day)
        in_month = day.month == @date.month
        today = day == Date.current
        events = by_day[day] || []
        reminders = reminders_by_day[day] || []
        snoozed = snoozed_by_day[day] || []
        scheduled = scheduled_by_day[day] || []

        div(
          data: {
            "new-url": helpers.new_calendar_event_path(date: day.iso8601, view: "month"),
            "calendar-month-dnd-target": "day",
            "date": day.iso8601
          },
          class: class_names(
            "min-h-[84px] sm:min-h-[104px] cursor-pointer border-b border-r border-border p-1 transition-colors hover:bg-muted/20 [&:nth-child(7n)]:border-r-0",
            in_month ? "bg-card" : "bg-muted/30"
          )
        ) do
          a(href: helpers.new_calendar_event_path(date: day.iso8601, view: "month"),
            data: { "calendar-event-modal-open": helpers.new_calendar_event_path(date: day.iso8601, view: "month") },
            class: class_names(
              "flex items-center justify-center w-6 h-6 mb-0.5 text-xs rounded-full transition-colors hover:bg-muted",
              today ? "bg-primary text-primary-foreground font-semibold" : (in_month ? "text-gray-700" : "text-gray-400")
            )) { day.day.to_s }

          div(class: "space-y-0.5") do
            shown = events.first(MAX_CHIPS)
            shown.each { |event| render_chip(event) }
            slots = MAX_CHIPS - shown.size
            reminders.first(slots).each { |reminder| render Campbooks::Calendar::ReminderChip.new(reminder: reminder) } if slots.positive?
            slots -= [ reminders.size, slots ].min
            snoozed.first(slots).each { |thread| render Campbooks::Calendar::SnoozedChip.new(thread: thread) } if slots.positive?
            slots -= [ snoozed.size, slots ].min
            scheduled.first(slots).each { |email| render Campbooks::Calendar::ScheduledEmailChip.new(scheduled_email: email) } if slots.positive?
            overflow = events.size + reminders.size + snoozed.size + scheduled.size - MAX_CHIPS
            div(class: "px-1 text-[10px] text-gray-400") { "+#{overflow}" } if overflow.positive?
          end
        end
      end

      def render_chip(event)
        color = event.display_color
        # Writable events become drag targets (cross-day reschedule); read-only
        # events stay plain links, matching DayGrid/WeekTimeGrid.
        drag = event.calendar.is_writable
        a(href: helpers.edit_calendar_event_path(event),
          data: {
            "calendar-event-modal-open": helpers.edit_calendar_event_path(event)
          }.merge(drag ? {
            "calendar-month-dnd-target": "event",
            "event-id": event.id,
            # App-zone wall-clock (no zone) so the client shifts only the date and the
            # server re-parses the same wall-clock — avoids a browser-vs-app TZ skew.
            "start-at": event.start_at.strftime("%Y-%m-%dT%H:%M"),
            "end-at": (event.end_at || event.start_at).strftime("%Y-%m-%dT%H:%M")
          } : {}),
          class: "block rounded px-1 py-0.5 text-[10px] sm:text-[11px] leading-tight",
          style: "background-color: #{color}; color: #{contrast_on(color)}",
          title: event.title) do
          span(class: "flex min-w-0 items-center gap-0.5") do
            type_icon(event, "h-2.5 w-2.5 flex-shrink-0")
            span(class: "min-w-0 truncate") { event.all_day ? event.title.to_s : "#{l(event.start_at, format: :clock)} #{event.title}" }
          end
        end
      end
    end
  end
end
