module Tools
  # Creates a calendar event from an email and pushes it to the provider. Backs
  # the `create_calendar_event` action across the inbox, Cmd+K, Scout, and
  # workflows. Event details come from the caller's args, falling back to a
  # heuristic extraction of the email (Ai::EventExtractor). Returns the created
  # CalendarEvent, or nil when the user has no writable calendar.
  class CreateCalendarEvent
    def self.call(email_message, args = {}, user: Current.user)
      new(email_message, args, user).call
    end

    def initialize(email_message, args, user)
      @email = email_message
      @args = (args || {}).with_indifferent_access
      @user = user
    end

    def call
      calendar = target_calendar
      return nil unless calendar

      details = Ai::EventExtractor.new(@email).extract
      start_at = parse_time(@args[:start_time]) || details.start_at
      end_at   = parse_time(@args[:end_time]) || details.end_at || (start_at + 3600)

      event = calendar.calendar_events.new(
        provider_event_id: "local-#{SecureRandom.uuid}",
        title: @args[:title].presence || details.title,
        description: @args[:description].presence,
        location: @args[:location].presence || details.location,
        start_at: start_at,
        end_at: end_at,
        all_day: false,
        status: :confirmed,
        outbound_pending: true,
        source_email_message: @email
      )
      return nil unless event.save

      Calendars::EventWriteJob.perform_later(event.id, "create")
      event
    end

    private

    # The user's primary writable calendar (or a specific one if passed), scoped
    # to accounts they can write to. nil ⇒ no calendar to create on.
    def target_calendar
      scope = Calendar.where(calendar_account: @user&.writable_calendar_accounts, is_writable: true, syncing: true)
      if @args[:calendar_id].present?
        scope.find_by(id: @args[:calendar_id]) || scope.order(is_primary: :desc).first
      else
        scope.order(is_primary: :desc).first
      end
    end

    def parse_time(val)
      return nil if val.blank?
      Time.zone.parse(val.to_s)
    rescue ArgumentError
      nil
    end
  end
end
