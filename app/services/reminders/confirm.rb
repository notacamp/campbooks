module Reminders
  # Confirms a pending reminder into a real CalendarEvent (the suggest-and-confirm
  # payoff). Source-agnostic sibling of Tools::CreateCalendarEvent — works from the
  # reminder's own structured data, for both email- and document-sourced reminders.
  #
  # Degrades gracefully: with no writable calendar the reminder is still marked
  # confirmed (calendar_event stays nil) so it remains visible in-app and on the
  # calendar's suggestion layer.
  class Confirm
    Result = Data.define(:success, :calendar_event, :error) do
      def success? = success
      def calendar? = !calendar_event.nil?
    end

    def self.call(reminder, user: Current.user)
      new(reminder, user).call
    end

    def initialize(reminder, user)
      @reminder = reminder
      @user = user
    end

    def call
      calendar = target_calendar

      unless calendar
        @reminder.update!(status: :confirmed, confirmed_by: @user)
        Events.publish("reminder.confirmed", subject: @reminder, payload: { "title" => @reminder.title, "due_at" => @reminder.due_at&.iso8601 })
        return Result.new(success: true, calendar_event: nil, error: nil)
      end

      event = calendar.calendar_events.create!(
        provider_event_id:    "local-#{SecureRandom.uuid}",
        title:                @reminder.title,
        description:          @reminder.description,
        start_at:             @reminder.due_at,
        end_at:               end_at,
        all_day:              @reminder.all_day,
        status:               :confirmed,
        outbound_pending:     true,
        source_email_message: email_source
      )

      Calendars::EventWriteJob.perform_later(event.id, "create")

      @reminder.update!(status: :confirmed, calendar_event: event, confirmed_by: @user)
      Events.publish("reminder.confirmed", subject: @reminder, payload: { "title" => @reminder.title, "due_at" => @reminder.due_at&.iso8601 })
      Result.new(success: true, calendar_event: event, error: nil)
    rescue ActiveRecord::RecordInvalid => e
      Result.new(success: false, calendar_event: nil, error: e.message)
    end

    private

    def end_at
      @reminder.all_day? ? @reminder.due_at + 1.day : @reminder.due_at + 1.hour
    end

    # Prefer the calendar of the SAME mailbox the reminder came from, so the event
    # pushes back to that Google/Zoho calendar (it shares the source email account's
    # OAuth grant). Fall back to the user's primary writable calendar.
    def target_calendar
      source_account_calendar || primary_writable_calendar
    end

    def source_account_calendar
      account = source_calendar_account
      return nil unless account && @user&.writable_calendar_accounts&.exists?(account.id)

      account.calendars.where(is_writable: true, syncing: true).order(is_primary: :desc).first
    end

    def primary_writable_calendar
      return nil unless @user
      Calendar.where(calendar_account: @user.writable_calendar_accounts, is_writable: true, syncing: true)
              .order(is_primary: :desc).first
    end

    # The CalendarAccount provisioned from the reminder's source mailbox — matched on
    # email_address + provider (Calendars::AccountProvisioner pairs them that way).
    def source_calendar_account
      ea = source_email_account
      return nil unless ea && CalendarAccount.providers.key?(ea.provider)

      CalendarAccount.find_by(workspace_id: ea.workspace_id, email_address: ea.email_address, provider: ea.provider)
    end

    # The email account behind the reminder: the source email, or the email a
    # source document was attached to.
    def source_email_account
      case @reminder.source
      when EmailMessage then @reminder.source.email_account
      when Document     then @reminder.source.email_messages.first&.email_account
      end
    end

    # CalendarEvent links back to an email source only; document-sourced reminders
    # omit it (no Document FK on CalendarEvent — the reminder keeps the source link).
    def email_source
      @reminder.source if @reminder.source_type == "EmailMessage"
    end
  end
end
