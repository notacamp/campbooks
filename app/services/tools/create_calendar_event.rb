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

      explicit_start = parse_time(@args[:start_time])

      # Idempotent: if Scout, the feed reminder card, or a previous click already
      # created an event for this email, return that one instead of stacking a
      # duplicate. The only case that still creates a second event is an explicit
      # start time on a different calendar day (a genuinely distinct commitment).
      if (existing = CalendarEvent.duplicate_for(email: @email, start_at: explicit_start))
        cross_link_reminders(existing)
        return existing
      end

      details = Ai::EventExtractor.new(@email).extract
      start_at = explicit_start || details.start_at
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
      # Auto-classify into an EventType (whose icon marks it) in the background.
      # The short delay lets the create write swap in the real provider id first.
      # type_status defaults to pending → the job runs.
      EventClassificationJob.set(wait: 10.seconds).perform_later(event.id)
      cross_link_reminders(event)
      # Leave a trace in the email's discussion: Scout notes the event it just
      # created, linking back to it. Best-effort — never blocks event creation.
      announce_to_discussion(event)
      event
    end

    private

    # Confirm and link any same-day pending reminder staged from this same email, so a
    # meeting that surfaced both a Reminder card and a Scout "Create event" button doesn't
    # leave a now-redundant reminder behind. Mirrors Reminders::Confirm's confirm effects.
    def cross_link_reminders(event)
      return unless event.start_at

      Reminder.where(source_type: "EmailMessage", source_id: @email.id, status: :pending)
              .where("due_at::date = ?::date", event.start_at)
              .find_each do |reminder|
        reminder.update!(calendar_event: event, status: :confirmed, confirmed_by: @user)
        Events.publish("reminder.confirmed", subject: reminder,
                       payload: { "title" => reminder.title, "due_at" => reminder.due_at&.iso8601 })
      end
    end

    def announce_to_discussion(event)
      return unless @email

      Discussions::ScoutAnnouncer.announce(email_message: @email) do
        I18n.t(
          "discussions.scout.calendar_event_created",
          title: event.title,
          url: Rails.application.routes.url_helpers.calendar_event_path(event),
          at: I18n.l(event.start_at, format: :at_short)
        )
      end
    end

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
