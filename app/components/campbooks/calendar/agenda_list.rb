module Campbooks
  module Calendar
    class AgendaList < Campbooks::Base
      def initialize(events:, reminders: [], snoozed_threads: [], scheduled_emails: [])
        @events = events.to_a
        @reminders = reminders.to_a
        @snoozed_threads = snoozed_threads.to_a
        @scheduled_emails = scheduled_emails.to_a
      end

      def view_template
        if @events.empty? && @reminders.empty? && @snoozed_threads.empty? && @scheduled_emails.empty?
          render Campbooks::EmptyState.new(variant: :card, title: t(".empty_title"), description: t(".empty_desc"))
          return
        end

        div(class: "space-y-6") do
          days.each do |day|
            section do
              h3(class: "text-xs font-semibold uppercase tracking-wide text-gray-400 mb-2") { day_label(day) }
              div(class: "space-y-1.5") do
                (events_by_day[day] || []).each { |event| render_event_row(event) }
                (reminders_by_day[day] || []).each { |reminder| render Campbooks::Calendar::ReminderChip.new(reminder: reminder, variant: :row) }
                (snoozed_by_day[day] || []).each { |thread| render Campbooks::Calendar::SnoozedRow.new(thread: thread) }
                (scheduled_by_day[day] || []).each { |email| render Campbooks::Calendar::ScheduledEmailRow.new(scheduled_email: email) }
              end
            end
          end
        end
      end

      private

      # Wrap deletable, non-recurring events in a swipe-to-delete shell.
      # Skip the wrapper for recurring series (the recurrence modal gap) and for
      # events on read-only calendars (would 403 on delete).
      def render_event_row(event)
        if event.recurring? || !event_swipe_deletable?(event)
          render Campbooks::Calendar::EventRow.new(event: event)
        else
          render(Campbooks::Swipeable.new(
            id: helpers.dom_id(event, :agenda_item),
            left: [
              { key: "delete", label: t("components.swipeable.delete"), icon: :trash, color: "red",
                endpoint: helpers.calendar_event_path(event), method: "delete",
                params: { swipe: 1 }, removes: true,
                confirm: { title: t("components.swipeable.confirm_event_delete_title"),
                           message: t("components.swipeable.confirm_event_delete_message") } }
            ],
            right: []
          )) { render Campbooks::Calendar::EventRow.new(event: event) }
        end
      end

      # An event is swipe-deletable when the calendar is writable and the current
      # user has write access to the calendar account.
      def event_swipe_deletable?(event)
        event.calendar.is_writable &&
          event.calendar_account.writable_by?(helpers.current_user)
      end

      def events_by_day
        @events_by_day ||= @events.group_by { |e| e.start_at.to_date }
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

      def days
        (events_by_day.keys | reminders_by_day.keys | snoozed_by_day.keys | scheduled_by_day.keys).sort
      end

      def day_label(date)
        if date == Date.current
          t(".today")
        elsif date == Date.current + 1
          t(".tomorrow")
        else
          l(date, format: :full)
        end
      end
    end
  end
end
