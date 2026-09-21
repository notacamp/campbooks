# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Builds the screen-shaped payload for GET /api/app/calendar from a
      # Calendars::PageData::Result. Returns all four event collections plus the
      # accounts sidebar in one round trip.
      #
      # snoozed_threads and scheduled_emails are omitted (nil) when the workspace
      # lacks the :email_scheduling entitlement — Calendars::PageData already
      # returns empty collections in that case; we follow its lead.
      class CalendarPageSerializer
        def initialize(data, user:)
          @data = data
          @user = user
        end

        def as_json
          {
            view: @data.respond_to?(:view) ? @data.view : nil,
            range: {
              start: @data.range.begin.iso8601,
              end: @data.range.end.iso8601
            },
            prev_date: @data.prev_date.iso8601,
            next_date: @data.next_date.iso8601,
            has_calendars: @data.has_calendars,
            calendar_accounts: accounts_data,
            events: events_data,
            reminders: reminders_data,
            snoozed_threads: snoozed_threads_data,
            scheduled_emails: scheduled_emails_data
          }
        end

        private

        def accounts_data
          @data.calendar_accounts.map do |account|
            CalendarAccountSerializer.new(
              account,
              user: @user,
              managed_ids: @data.managed_calendar_account_ids
            ).as_json
          end
        end

        def events_data
          @data.events.map { |e| CalendarEventSerializer.new(e).as_json }
        end

        def reminders_data
          @data.reminders.map do |r|
            {
              id: r.id,
              title: r.title,
              due_at: r.due_at&.iso8601,
              confirmed: r.calendar_event_id.present?
            }
          end
        end

        def snoozed_threads_data
          return nil unless @data.snoozed_threads.respond_to?(:map)
          @data.snoozed_threads.map do |thread|
            {
              id: thread.id,
              subject: thread.subject,
              snoozed_until: thread.snoozed_until&.iso8601
            }
          end
        end

        def scheduled_emails_data
          return nil unless @data.scheduled_emails.respond_to?(:map)
          @data.scheduled_emails.map do |se|
            {
              id: se.id,
              subject: se.subject,
              scheduled_at: (se.respond_to?(:next_occurrence_at) ? (se.next_occurrence_at || se.scheduled_at) : se.scheduled_at)&.iso8601
            }
          end
        end
      end
    end
  end
end
