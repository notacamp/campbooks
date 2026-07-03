module Campbooks
  module Calendar
    class WeekGrid < Campbooks::Base
      include TypeIcon

      def initialize(date:, events:, reminders: [], snoozed_threads: [], scheduled_emails: [])
        @date = date
        @events = events.to_a
        @reminders = reminders.to_a
        @snoozed_threads = snoozed_threads.to_a
        @scheduled_emails = scheduled_emails.to_a
      end

      def view_template
        div(class: "grid grid-cols-1 gap-2 sm:grid-cols-7") do
          days.each { |day| render_day(day) }
        end
      end

      private

      def days
        (@date.beginning_of_week..@date.end_of_week).to_a
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

      def render_day(day)
        today = day == Date.current
        events = by_day[day] || []
        reminders = reminders_by_day[day] || []
        snoozed = snoozed_by_day[day] || []
        scheduled = scheduled_by_day[day] || []
        empty = events.empty? && reminders.empty? && snoozed.empty? && scheduled.empty?

        div(class: class_names(
          "min-h-[120px] rounded-xl border p-2.5",
          today ? "border-transparent bg-muted/40 ring-1 ring-primary/30" : "border-border/70 bg-card"
        )) do
          div(class: "mb-1.5 flex items-center justify-between") do
            span(class: class_names("text-xs font-semibold uppercase tracking-wide", today ? "text-foreground" : "text-muted-foreground")) { t("date.abbr_day_names")[day.wday] }
            span(class: class_names(
              "flex h-6 min-w-6 items-center justify-center rounded-full px-1 text-xs",
              today ? "bg-primary font-semibold text-primary-foreground" : "text-gray-500"
            )) { day.day.to_s }
          end

          if empty
            a(href: helpers.new_calendar_event_path(date: day.iso8601, view: "week"),
              data: { "calendar-event-modal-open": helpers.new_calendar_event_path(date: day.iso8601, view: "week") },
              class: "flex items-center gap-1 rounded-md px-1 py-1 text-[11px] text-muted-foreground/70 transition-colors hover:bg-muted hover:text-foreground") { t(".add") }
          else
            div(class: "space-y-1") do
              events.each { |event| render_chip(event) }
              reminders.each { |reminder| render Campbooks::Calendar::ReminderChip.new(reminder: reminder) }
              snoozed.each { |thread| render Campbooks::Calendar::SnoozedChip.new(thread: thread) }
              scheduled.each { |email| render Campbooks::Calendar::ScheduledEmailChip.new(scheduled_email: email) }
            end
          end
        end
      end

      def render_chip(event)
        color = event.display_color
        a(href: helpers.edit_calendar_event_path(event),
          data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(event) },
          class: "block rounded-md px-1.5 py-1 text-[11px] leading-tight shadow-sm",
          style: "background-color: #{color}; color: #{contrast_on(color)}",
          title: event.title) do
          span(class: "flex min-w-0 items-center gap-1 font-semibold") do
            type_icon(event, "h-3 w-3 flex-shrink-0")
            span(class: "min-w-0 truncate") { event.title.presence || t(".untitled") }
          end
          span(class: "block opacity-90 tabular-nums") { l(event.start_at, format: :clock) } unless event.all_day
        end
      end
    end
  end
end
