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
      remote = @client.create_event(@calendar, attrs_for_provider)
      apply_remote!(remote)
    end

    def update(scope)
      target = target_provider_id(scope)
      with_conflict_retry do |etag|
        @client.update_event(@calendar, target, attrs_for_provider, etag: etag)
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

      @event.update!(
        provider_event_id: remote[:provider_event_id].presence || @event.provider_event_id,
        provider_etag: remote[:provider_etag],
        provider_sequence: remote[:provider_sequence],
        html_link: remote[:html_link].presence || @event.html_link,
        conference_url: remote[:conference_url].presence || @event.conference_url,
        outbound_pending: false
      )
    end

    def target_provider_id(scope)
      if scope.to_sym == :all && @event.recurring_event_provider_id.present?
        @event.recurring_event_provider_id
      else
        @event.provider_event_id
      end
    end

    def attrs_for_provider
      {
        title: @event.title,
        description: @event.description,
        location: @event.location,
        start_at: @event.start_at,
        end_at: @event.end_at,
        all_day: @event.all_day,
        time_zone: @event.start_time_zone,
        color: @event.provider_color,
        attendees: @event.attendees
      }
    end

    PROVIDER_RSVP = {
      "accepted" => "accepted", "declined" => "declined",
      "tentative" => "tentative", "needs_action" => "needsAction"
    }.freeze

    # The full attendee list with the account holder's own responseStatus set —
    # providers expect the whole list on an RSVP patch, not just the delta.
    def attendees_with_self_response
      mine = @account.email_address.to_s.downcase
      status = PROVIDER_RSVP[@event.rsvp_status.to_s] || "needsAction"
      list = Array(@event.attendees).map(&:dup)
      self_row = list.find { |a| a["email"].to_s.downcase == mine }
      if self_row
        self_row["responseStatus"] = status
      else
        list << { "email" => @account.email_address, "responseStatus" => status, "self" => true }
      end
      list.map { |a| { email: a["email"], displayName: a["name"], responseStatus: a["responseStatus"] }.compact }
    end
  end
end
