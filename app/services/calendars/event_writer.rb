module Calendars
  # Pushes a locally-saved CalendarEvent to its provider (the outbound half of
  # two-way sync). Invoked async from Calendars::EventWriteJob after the controller
  # has persisted the event with `outbound_pending: true`.
  #
  # Loop-avoidance: on success we store the provider's returned id/etag immediately
  # and clear `outbound_pending`, so the next inbound sync sees a matching etag and
  # skips the row (plan Risk 1). For a fresh create the controller assigns a temp
  # `provider_event_id` ("local-…"); create() swaps in the real one here.
  class EventWriter
    def initialize(event)
      @event = event
      @calendar = event.calendar
      @account = @calendar.calendar_account
      @client = @account.calendar_client
    end

    # operation: :create | :update | :delete | :rsvp
    # scope: :this targets the single (instance) event; :all targets the recurring
    #        series (the provider series id) for an "all events" edit.
    def call(operation, scope: :this)
      case operation.to_sym
      when :create then create
      when :update then update(scope)
      when :delete then delete(scope)
      when :rsvp   then rsvp
      else raise ArgumentError, "unknown calendar write operation: #{operation}"
      end
    end

    private

    def create
      remote = @client.create_event(@calendar, attrs_for_provider(recurrence: true))
      apply_remote!(remote)
    end

    def update(scope)
      target = target_provider_id(scope)
      # Only carry the series rule when writing the series itself — editing a local
      # master, or an "all events" edit of a provider series; a single-instance edit
      # must not restamp the recurrence.
      with_conflict_retry do |etag|
        @client.update_event(@calendar, target, attrs_for_provider(recurrence: scope.to_sym == :all || @event.series_master?), etag: etag)
      end
    end

    def delete(scope)
      target = target_provider_id(scope)
      @client.delete_event(@calendar, target, etag: @event.provider_etag)
      @event.update!(status: :cancelled, outbound_pending: false)
    end

    def rsvp
      with_conflict_retry do |etag|
        @client.patch_rsvp(@calendar, @event.provider_event_id, attendees: attendees_with_self_response, etag: etag)
      end
    end

    # Runs the write with the stored etag as an If-Match guard. On a 412 the event
    # changed remotely since we loaded it: re-fetch to adopt the fresh etag, then
    # retry once without the guard so the user's edit wins (last-write-wins).
    def with_conflict_retry
      remote = yield(@event.provider_etag)
      apply_remote!(remote)
    rescue Calendars::ConflictError
      fresh = @client.get_event(@calendar, @event.provider_event_id)
      @event.update_columns(provider_etag: fresh[:provider_etag]) if fresh
      remote = yield(nil)
      apply_remote!(remote)
    end

    def apply_remote!(remote)
      return @event.update_columns(outbound_pending: false) unless remote

      attrs = {
        provider_event_id: remote[:provider_event_id].presence || @event.provider_event_id,
        provider_etag: remote[:provider_etag],
        provider_sequence: remote[:provider_sequence],
        html_link: remote[:html_link].presence || @event.html_link,
        conference_url: remote[:conference_url].presence || @event.conference_url,
        outbound_pending: false
      }
      # Adopt the provider's authority on organizer + guest list. Storing the
      # etag below makes the next inbound sync SKIP this row (loop-avoidance),
      # so anything not persisted here stays stale until the event changes
      # remotely — that's how app-created events were stuck with
      # is_organizer: false and never got the guests editor.
      attrs[:is_organizer] = remote[:is_organizer] unless remote[:is_organizer].nil?
      attrs[:attendees] = remote[:attendees] unless remote[:attendees].nil?
      @event.update!(attrs)
    end

    def target_provider_id(scope)
      if scope.to_sym == :all && @event.recurring_event_provider_id.present?
        @event.recurring_event_provider_id
      else
        @event.provider_event_id
      end
    end

    def attrs_for_provider(recurrence: false)
      attrs = {
        title: @event.title,
        description: @event.description,
        location: @event.location,
        start_at: @event.start_at,
        end_at: @event.end_at,
        all_day: @event.all_day,
        time_zone: @event.start_time_zone
      }
      attrs[:attendees] = canonical_attendees if push_attendees?
      attrs[:rrule] = @event.rrule if recurrence && @event.rrule.present?
      attrs
    end

    # The guest list is written back only for events we own — the organizer's
    # copy, or an app-created event on its first push (the provider hasn't told
    # us we're the organizer yet). Pushing the list on an invite we merely
    # received would clobber the organizer's guest list with our stale copy.
    def push_attendees?
      @event.is_organizer? || @event.provider_event_id.to_s.start_with?("local-")
    end

    # Attendee rows in one symbol-keyed shape regardless of how they were stored
    # (jsonb rows are string-keyed; rsvp_status may be in the provider's own
    # vocabulary — the clients map it). Rows without an email are dropped.
    def canonical_attendees
      Array(@event.attendees).filter_map do |a|
        next { email: a } if a.is_a?(String)
        row = a.transform_keys(&:to_s)
        email = row["email"].presence
        next unless email
        { email: email, name: row["name"].presence, rsvp_status: row["rsvp_status"].presence }.compact
      end
    end

    # The full attendee list with the account holder's own rsvp_status set —
    # providers expect the whole list on an RSVP patch, not just the delta.
    def attendees_with_self_response
      mine = @account.email_address.to_s.downcase
      list = canonical_attendees
      self_row = list.find { |a| a[:email].to_s.downcase == mine }
      unless self_row
        self_row = { email: @account.email_address }
        list << self_row
      end
      self_row[:rsvp_status] = @event.rsvp_status.to_s
      list
    end
  end
end
