module Google
  # Google Calendar API v3 client — the calendar-side sibling of Google::MailClient.
  # Normalizes responses to a common event hash (the keys map to CalendarEvent
  # columns) so the sync job and EventWriter stay provider-agnostic. The Faraday
  # connection is rebuilt per request so the fresh cached access token is read.
  class CalendarClient
    BASE_URL = "https://www.googleapis.com/calendar/v3"
    PAGE_SIZE = 250

    def initialize(calendar_account)
      @account = calendar_account
      @oauth = calendar_account.oauth_client
    end

    # --- Calendars ---

    # The account's calendars, normalized. Used at link time and on full sync.
    def calendar_list
      paginate("#{BASE_URL}/users/me/calendarList").map { |c| normalize_calendar(c) }
    end

    # --- Events: read ---

    # Incremental pull using the stored sync token. Returns changed/added/deleted
    # events. Raises Calendars::SyncTokenExpired on HTTP 410 so the caller can
    # fall back to a full re-sync.
    def list_events_incremental(calendar)
      fetch_events(calendar, syncToken: calendar.sync_token, singleEvents: true)
    end

    # Full pull over a bounded window (no sync token). singleEvents expands
    # recurrences into concrete instances; the last page carries nextSyncToken.
    def list_events_full(calendar, time_min:, time_max:)
      fetch_events(calendar,
                   timeMin: time_min.utc.iso8601,
                   timeMax: time_max.utc.iso8601,
                   singleEvents: true,
                   showDeleted: false)
    end

    def get_event(calendar, provider_event_id)
      response = connection.get(events_url(calendar, provider_event_id))
      return nil unless response.success?
      normalize_event(JSON.parse(response.body))
    end

    # --- Events: write ---

    def create_event(calendar, attrs)
      response = connection.post(events_url(calendar)) do |req|
        req.headers["Content-Type"] = "application/json"
        req.params["sendUpdates"] = "all"
        req.body = build_payload(attrs).to_json
      end
      raise_for_status!(response, "create_event")
      normalize_event(JSON.parse(response.body))
    end

    def update_event(calendar, provider_event_id, attrs, etag: nil)
      response = connection.patch(events_url(calendar, provider_event_id)) do |req|
        req.headers["Content-Type"] = "application/json"
        req.headers["If-Match"] = etag if etag.present?
        req.params["sendUpdates"] = "all"
        req.body = build_payload(attrs).to_json
      end
      raise Calendars::ConflictError, "etag mismatch on #{provider_event_id}" if response.status == 412
      raise_for_status!(response, "update_event")
      normalize_event(JSON.parse(response.body))
    end

    def delete_event(calendar, provider_event_id, etag: nil)
      response = connection.delete(events_url(calendar, provider_event_id)) do |req|
        req.headers["If-Match"] = etag if etag.present?
        req.params["sendUpdates"] = "all"
      end
      return true if [ 404, 410 ].include?(response.status) # already gone — treat as success
      raise Calendars::ConflictError, "etag mismatch on #{provider_event_id}" if response.status == 412
      raise_for_status!(response, "delete_event")
      true
    end

    # Set the account holder's attendance. Google wants the full attendee list on
    # patch, so the caller passes the merged list with `self`'s responseStatus set.
    def patch_rsvp(calendar, provider_event_id, attendees:, etag: nil)
      response = connection.patch(events_url(calendar, provider_event_id)) do |req|
        req.headers["Content-Type"] = "application/json"
        req.headers["If-Match"] = etag if etag.present?
        req.params["sendUpdates"] = "all"
        req.body = { attendees: attendees }.to_json
      end
      raise Calendars::ConflictError, "etag mismatch on #{provider_event_id}" if response.status == 412
      raise_for_status!(response, "patch_rsvp")
      normalize_event(JSON.parse(response.body))
    end

    # --- Push channels (watch) ---

    def watch_calendar(calendar, channel_id:, token:, address:, ttl_seconds: nil)
      body = { id: channel_id, type: "web_hook", address: address, token: token }
      body[:params] = { ttl: ttl_seconds.to_s } if ttl_seconds
      response = connection.post("#{events_url(calendar)}/watch") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = body.to_json
      end
      raise_for_status!(response, "watch_calendar")
      JSON.parse(response.body) # { "resourceId" =>, "expiration" => ms, ... }
    end

    def stop_channel(channel_id:, resource_id:)
      response = connection.post("#{BASE_URL}/channels/stop") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { id: channel_id, resourceId: resource_id }.to_json
      end
      response.success?
    end

    private

    def fetch_events(calendar, **params)
      events = []
      next_sync_token = nil
      page_token = nil
      loop do
        query = params.merge(maxResults: PAGE_SIZE)
        query[:pageToken] = page_token if page_token
        response = connection.get(events_url(calendar), query)
        raise Calendars::SyncTokenExpired, "sync token expired for calendar #{calendar.id}" if response.status == 410
        raise_for_status!(response, "list_events")
        data = JSON.parse(response.body)
        (data["items"] || []).each { |e| events << normalize_event(e) }
        next_sync_token = data["nextSyncToken"] if data["nextSyncToken"]
        page_token = data["nextPageToken"]
        break unless page_token
      end
      { events: events, next_sync_token: next_sync_token }
    end

    def paginate(url, params = {})
      items = []
      page_token = nil
      loop do
        query = params.dup
        query[:pageToken] = page_token if page_token
        response = connection.get(url, query)
        raise_for_status!(response, "list")
        data = JSON.parse(response.body)
        items.concat(data["items"] || [])
        page_token = data["nextPageToken"]
        break unless page_token
      end
      items
    end

    def normalize_calendar(c)
      {
        provider_calendar_id: c["id"],
        name: c["summaryOverride"].presence || c["summary"],
        description: c["description"],
        color: c["backgroundColor"],
        time_zone: c["timeZone"],
        is_primary: c["primary"] == true,
        is_writable: %w[owner writer].include?(c["accessRole"])
      }
    end

    # Maps a Google event resource to the common hash consumed by the sync job.
    def normalize_event(e)
      start_at, start_tz, all_day = parse_time(e["start"])
      end_at, end_tz, _ = parse_time(e["end"])
      self_attendee = (e["attendees"] || []).find { |a| a["self"] }

      {
        provider_event_id: e["id"],
        title: e["summary"],
        description: e["description"],
        location: e["location"],
        html_link: e["htmlLink"],
        conference_url: conference_url(e),
        start_at: start_at,
        end_at: end_at,
        start_time_zone: start_tz,
        end_time_zone: end_tz,
        all_day: all_day,
        status: e["status"], # confirmed / tentative / cancelled
        rsvp_status: map_rsvp(self_attendee&.dig("responseStatus")),
        is_organizer: e.dig("organizer", "self") == true,
        attendees: normalize_attendees(e["attendees"]),
        provider_etag: e["etag"],
        provider_sequence: e["sequence"],
        rrule: extract_rrule(e["recurrence"]),
        recurring_event_provider_id: e["recurringEventId"],
        original_start_at: (parse_time(e["originalStartTime"]).first if e["originalStartTime"])
      }
    end

    def normalize_attendees(list)
      Array(list).map do |a|
        { "email" => a["email"], "name" => a["displayName"],
          "rsvp_status" => a["responseStatus"], "self" => a["self"] == true }
      end
    end

    # Returns [time(UTC), zone, all_day?]. An all-day event has `date`, a timed
    # one has `dateTime`.
    def parse_time(node)
      return [ nil, nil, false ] unless node
      if node["dateTime"].present?
        [ Time.parse(node["dateTime"]).utc, node["timeZone"], false ]
      elsif node["date"].present?
        [ Time.parse(node["date"]).utc, node["timeZone"], true ]
      else
        [ nil, nil, false ]
      end
    rescue ArgumentError
      [ nil, nil, false ]
    end

    def conference_url(e)
      return e["hangoutLink"] if e["hangoutLink"].present?
      e.dig("conferenceData", "entryPoints")&.find { |p| p["entryPointType"] == "video" }&.dig("uri")
    end

    def extract_rrule(recurrence)
      return nil unless recurrence.is_a?(Array)
      recurrence.find { |r| r.to_s.start_with?("RRULE:") }&.delete_prefix("RRULE:")
    end

    GOOGLE_RSVP = { "accepted" => "accepted", "declined" => "declined",
                    "tentative" => "tentative", "needsAction" => "needs_action" }.freeze

    def map_rsvp(status)
      GOOGLE_RSVP[status]
    end

    # Builds a Google event resource from the writer's attribute hash. Only keys
    # present in `attrs` are sent, so a partial patch stays partial.
    def build_payload(attrs)
      payload = {}
      payload[:summary] = attrs[:title] if attrs.key?(:title)
      payload[:description] = attrs[:description] if attrs.key?(:description)
      payload[:location] = attrs[:location] if attrs.key?(:location)

      if attrs[:all_day] && attrs[:start_at]
        payload[:start] = { date: attrs[:start_at].to_date.iso8601 }
        payload[:end] = { date: (attrs[:end_at] || attrs[:start_at]).to_date.iso8601 }
      elsif attrs[:start_at]
        zone = attrs[:time_zone].presence || "UTC"
        payload[:start] = { dateTime: attrs[:start_at].utc.iso8601, timeZone: zone }
        payload[:end]   = { dateTime: (attrs[:end_at] || (attrs[:start_at] + 3600)).utc.iso8601, timeZone: zone }
      end

      if attrs.key?(:attendees)
        payload[:attendees] = Array(attrs[:attendees]).map do |a|
          a.is_a?(String) ? { email: a } : { email: a[:email], displayName: a[:name] }.compact
        end
      end

      # Recurrence: Google takes an array of iCal lines; the DTSTART is implied by
      # the event's start above, so a bare "FREQ=WEEKLY" repeats on that weekday.
      # Google then expands the series and syncs the concrete instances back (we
      # pull with singleEvents=true).
      payload[:recurrence] = [ "RRULE:#{attrs[:rrule]}" ] if attrs[:rrule].present?
      payload
    end

    def events_url(calendar, event_id = nil)
      base = "#{BASE_URL}/calendars/#{CGI.escape(calendar.provider_calendar_id)}/events"
      event_id ? "#{base}/#{CGI.escape(event_id)}" : base
    end

    def raise_for_status!(response, context)
      return if response.success?
      raise AuthenticationError, "Google Calendar #{context} unauthorized" if response.status == 401
      Rails.logger.error("[Google::CalendarClient] #{context} failed: #{response.status} #{response.body.to_s[0..300]}")
      raise "Google Calendar #{context} failed: #{response.status}"
    end

    def connection
      Faraday.new do |f|
        f.request :url_encoded
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{@oauth.access_token}"
      end
    end
  end
end
