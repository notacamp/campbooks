module Campbooks
  module Calendar
    class WeekGrid < Campbooks::Base
      def initialize(date:, events:, reminders: [], snoozed_threads: [], scheduled_emails: [])
        @date = date
        @events = events.to_a
        @reminders = reminders.to_a
        @snoozed_threads = snoozed_threads.to_a
        @scheduled_emails = scheduled_emails.to_a
      end

      def view_template
        div(class: "grid grid-cols-1 sm:grid-cols-7 gap-2") do
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

        div(class: class_names("rounded-lg border border-border bg-card p-2 min-h-[120px]", ("ring-1 ring-primary/40" if today))) do
          div(class: "flex items-baseline justify-between mb-1.5") do
            span(class: class_names("text-xs font-semibold", today ? "text-primary" : "text-gray-700")) { t("date.abbr_day_names")[day.wday] }
            span(class: class_names("text-xs", today ? "text-primary font-semibold" : "text-gray-400")) { day.day.to_s }
          end

          if events.empty? && reminders.empty? && snoozed.empty? && scheduled.empty?
            a(href: helpers.new_calendar_event_path(date: day.iso8601, view: "week"),
              data: { "calendar-event-modal-open": helpers.new_calendar_event_path(date: day.iso8601, view: "week") },
              class: "block py-1 text-[11px] text-gray-300 hover:text-gray-500") { t(".add") }
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
          class: "block rounded px-1.5 py-1 text-[11px] leading-tight",
          style: "background-color: #{color}; color: #{contrast_on(color)}",
          title: event.title) do
          span(class: "block truncate font-medium") { event.title.presence || t(".untitled") }
          span(class: "block opacity-90") { l(event.start_at, format: :clock) } unless event.all_day
        end
      end
    end
  end
end
