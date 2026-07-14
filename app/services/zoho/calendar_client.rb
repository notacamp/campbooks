module Zoho
  # Zoho Calendar API v1 client — the calendar-side sibling of Zoho::MailClient,
  # exposing the same interface as Google::CalendarClient so the sync job and
  # EventWriter stay provider-agnostic.
  #
  # ⚠️ UNVERIFIED: written against the published Zoho Calendar API v1 shapes but
  # not yet exercised against a live grant (Zoho is the "then Zoho" provider, and
  # no Zoho calendar credentials are wired in dev yet). The Google path is the
  # tested one; revisit the field mappings + write payloads here once a real Zoho
  # calendar is connected. Reads are defensive (return empty on surprises).
  class CalendarClient
    BASE_URL = Region.calendar_api_url.freeze

    def initialize(calendar_account)
      @account = calendar_account
      @oauth = calendar_account.oauth_client
    end

    def calendar_list
      response = connection.get("#{BASE_URL}/calendars")
      unless response.success?
        # Reads stay defensive (return empty), but log the body — the Zoho path is
        # unverified against a live grant, and a silent 400 here is undiagnosable.
        Rails.logger.error("[Zoho::CalendarClient] calendar_list failed: #{response.status} #{response.body.to_s[0..300]}")
        return []
      end
      data = JSON.parse(response.body)
      Array(data["calendars"]).map { |c| normalize_calendar(c) }
    rescue JSON::ParserError => e
      Rails.logger.error("[Zoho::CalendarClient] calendar_list parse failed: #{e.message}")
      []
    end

    # Zoho has no incremental sync token, so an incremental pull is just a full
    # pull over the window. next_sync_token stays nil (the Calendar row falls back
    # to last_event_sync_at-based polling).
    #
    # This runs every minute, so it must stay a SINGLE request — chunking the
    # persisted multi-month window here would fan out to ~16 Zoho calls per
    # calendar per minute. Clamp to one 30-day window around now; changes outside
    # it are picked up by the 15-minute full sweep, which chunks the whole window.
    def list_events_incremental(calendar)
      window_start = calendar.sync_window_start || 30.days.ago
      window_end   = calendar.sync_window_end   || 365.days.from_now
      clamped_start = [ window_start.to_time, 7.days.ago.to_time ].max
      clamped_end   = [ window_end.to_time, clamped_start + SLICE_DAYS.days ].min
      list_events_full(calendar, time_min: clamped_start, time_max: clamped_end)
    end

    # Zoho caps the range parameter at 31 days per request. We slice [time_min,
    # time_max] into consecutive 30-day windows (one day of margin below the cap),
    # fetch each slice, concatenate the results, and deduplicate by
    # :provider_event_id so events that span a slice boundary appear only once.
    SLICE_DAYS = 30

    def list_events_full(calendar, time_min:, time_max:)
      all_events = {}
      slice_start = time_min.to_time
      slice_end_limit = time_max.to_time

      while slice_start < slice_end_limit
        slice_end = [ slice_start + SLICE_DAYS.days, slice_end_limit ].min
        range = { start: zoho_time(slice_start), end: zoho_time(slice_end) }.to_json
        response = connection.get("#{BASE_URL}/calendars/#{calendar.provider_calendar_id}/events", range: range)
        unless response.success?
          Rails.logger.error("[Zoho::CalendarClient] list_events failed: #{response.status} #{response.body.to_s[0..300]}")
          return { events: [], next_sync_token: nil }
        end
        data = JSON.parse(response.body)
        Array(data["events"]).each do |e|
          normalized = normalize_event(e)
          all_events[normalized[:provider_event_id]] ||= normalized
        end
        slice_start = slice_end
      end

      { events: all_events.values, next_sync_token: nil }
    rescue JSON::ParserError => e
      Rails.logger.error("[Zoho::CalendarClient] list_events parse failed: #{e.message}")
      { events: [], next_sync_token: nil }
    end

    def get_event(calendar, provider_event_id)
      response = connection.get("#{BASE_URL}/calendars/#{calendar.provider_calendar_id}/events/#{provider_event_id}")
      return nil unless response.success?
      data = JSON.parse(response.body)
      event = data["events"] ? Array(data["events"]).first : data
      event && normalize_event(event)
    end

    def create_event(calendar, attrs)
      response = connection.post("#{BASE_URL}/calendars/#{calendar.provider_calendar_id}/events") do |req|
        req.body = { eventdata: build_payload(attrs).to_json }
      end
      raise_for_status!(response, "create_event")
      parse_single(response)
    end

    def update_event(calendar, provider_event_id, attrs, etag: nil)
      response = connection.put("#{BASE_URL}/calendars/#{calendar.provider_calendar_id}/events/#{provider_event_id}") do |req|
        req.headers["etag"] = etag.to_s if etag.present?
        req.body = { eventdata: build_payload(attrs).to_json }
      end
      raise Calendars::ConflictError, "etag mismatch on #{provider_event_id}" if response.status == 412
      raise_for_status!(response, "update_event")
      parse_single(response)
    end

    def delete_event(calendar, provider_event_id, etag: nil)
      resolved_etag = etag.presence || fetch_event_etag(calendar.provider_calendar_id, provider_event_id)
      response = connection.delete("#{BASE_URL}/calendars/#{calendar.provider_calendar_id}/events/#{provider_event_id}") do |req|
        req.headers["etag"] = resolved_etag.to_s if resolved_etag.present?
      end
      return true if [ 404, 410 ].include?(response.status)
      raise Calendars::ConflictError, "etag mismatch on #{provider_event_id}" if response.status == 412
      raise_for_status!(response, "delete_event")
      true
    end

    def patch_rsvp(calendar, provider_event_id, attendees:, etag: nil)
      # Zoho RSVP is set per-attendee; for v1 we re-send the event with the merged
      # attendee list, mirroring the Google patch contract.
      update_event(calendar, provider_event_id, { attendees: attendees }, etag: etag)
    end

    def watch_calendar(*)
      raise NotImplementedError, "Zoho calendar push not supported in v1 (polling only)"
    end

    def stop_channel(*)
      false
    end

    private

    def normalize_calendar(c)
      {
        provider_calendar_id: c["uid"],
        name: c["name"],
        description: c["description"],
        color: c["color"],
        time_zone: c["timezone"],
        is_primary: c["isdefault"] == true,
        is_writable: c["privilege"].to_s != "read"
      }
    end

    def normalize_event(e)
      dt = e["dateandtime"] || {}
      start_at = parse_zoho_time(dt["start"])
      end_at   = parse_zoho_time(dt["end"])
      {
        provider_event_id: e["uid"],
        title: e["title"],
        description: e["description"],
        location: e["location"],
        html_link: e["vieweventurl"],
        conference_url: nil,
        start_at: start_at,
        end_at: end_at,
        start_time_zone: dt["timezone"],
        end_time_zone: dt["timezone"],
        all_day: e["isallday"] == true,
        status: e["iscancelled"] == true ? "cancelled" : "confirmed",
        rsvp_status: nil,
        is_organizer: e["isorganizer"] == true,
        attendees: Array(e["attendees"]).map { |a| { "email" => a["email"], "name" => a["dname"], "rsvp_status" => a["status"] } },
        provider_etag: e["etag"],
        provider_sequence: nil,
        rrule: e["rrule"].presence,
        recurring_event_provider_id: e["rrule"].present? ? e["uid"] : nil,
        original_start_at: nil
      }
    end

    def build_payload(attrs)
      payload = {}
      payload[:title] = attrs[:title] if attrs.key?(:title) && !attrs[:title].nil?
      payload[:description] = attrs[:description] if attrs.key?(:description) && !attrs[:description].nil?
      payload[:location] = attrs[:location] if attrs.key?(:location) && !attrs[:location].nil?
      if attrs[:start_at]
        payload[:dateandtime] = {
          start: zoho_time(attrs[:start_at]),
          end: zoho_time(attrs[:end_at] || (attrs[:start_at] + 3600)),
          timezone: attrs[:time_zone].presence || "UTC"
        }
      end
      payload[:isallday] = true if attrs[:all_day]
      if attrs.key?(:attendees)
        payload[:attendees] = Array(attrs[:attendees]).filter_map { |a| attendee_payload(a) }
      end
      # Zoho carries the recurrence as a bare RRULE string (mirrors normalize_event
      # reading e["rrule"]). ⚠️ Unverified against a live Zoho grant.
      payload[:rrule] = attrs[:rrule] if attrs[:rrule].present?
      payload
    end

    # ICS-style participation statuses, keyed by our rsvp_status enum values
    # (mirrors normalize_event reading a["status"]). ⚠️ Unverified against a
    # live Zoho grant, like the rest of the write payloads.
    ZOHO_RSVP_OUT = { "accepted" => "ACCEPTED", "declined" => "DECLINED",
                      "tentative" => "TENTATIVE", "needs_action" => "NEEDS-ACTION" }.freeze

    # One attendee for an outbound payload — canonical symbol-keyed rows from
    # EventWriter, raw string-keyed jsonb rows, or a bare email string. Statuses
    # already in Zoho's ICS vocabulary (stored by inbound sync) pass through.
    def attendee_payload(a)
      return { email: a } if a.is_a?(String)
      row = a.transform_keys(&:to_s)
      email = row["email"].presence
      return nil unless email
      { email: email, dname: row["name"].presence, status: attendee_status(row["rsvp_status"]) }.compact
    end

    def attendee_status(value)
      v = value.to_s
      ZOHO_RSVP_OUT[v] || ZOHO_RSVP_OUT.values.find { |ics| ics.casecmp?(v) }
    end

    # Fetch the current etag for an event so delete_event can send it as a header
    # even when the caller did not supply one. Returns nil on any failure so the
    # caller can proceed without an etag (matching the previous behaviour).
    def fetch_event_etag(calendar_id, provider_event_id)
      response = connection.get("#{BASE_URL}/calendars/#{calendar_id}/events/#{provider_event_id}")
      return nil unless response.success?
      data = JSON.parse(response.body)
      events = data["events"] ? Array(data["events"]) : []
      events.first&.dig("etag")
    rescue StandardError => e
      Rails.logger.warn("[Zoho::CalendarClient] fetch_event_etag failed for #{provider_event_id}: #{e.message}")
      nil
    end

    def parse_single(response)
      data = JSON.parse(response.body)
      event = data["events"] ? Array(data["events"]).first : data
      normalize_event(event || {})
    end

    # Zoho's basic-ISO timestamp, e.g. "20260101T100000Z".
    def zoho_time(time)
      time&.utc&.strftime("%Y%m%dT%H%M%SZ")
    end

    def parse_zoho_time(str)
      return nil if str.blank?
      Time.parse(str).utc
    rescue ArgumentError
      nil
    end

    def raise_for_status!(response, context)
      return if response.success?
      raise AuthenticationError, "Zoho Calendar #{context} unauthorized" if response.status == 401
      Rails.logger.error("[Zoho::CalendarClient] #{context} failed: #{response.status} #{response.body.to_s[0..300]}")
      raise "Zoho Calendar #{context} failed: #{response.status}"
    end

    def connection
      Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "zoho_calendar", expected_statuses: [ 410, 412 ], workspace: -> { @account.try(:workspace_id) }
        f.request :url_encoded
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Zoho-oauthtoken #{@oauth.access_token}"
      end
    end
  end
end
