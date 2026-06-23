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
    BASE_URL = "https://calendar.zoho.eu/api/v1"

    def initialize(calendar_account)
      @account = calendar_account
      @oauth = calendar_account.oauth_client
    end

    def calendar_list
      response = connection.get("#{BASE_URL}/calendars")
      return [] unless response.success?
      data = JSON.parse(response.body)
      Array(data["calendars"]).map { |c| normalize_calendar(c) }
    rescue JSON::ParserError => e
      Rails.logger.error("[Zoho::CalendarClient] calendar_list parse failed: #{e.message}")
      []
    end

    # Zoho has no incremental sync token, so an incremental pull is just a full
    # pull over the window. next_sync_token stays nil (the Calendar row falls back
    # to last_event_sync_at-based polling).
    def list_events_incremental(calendar)
      window_start = calendar.sync_window_start || 30.days.ago
      window_end   = calendar.sync_window_end   || 365.days.from_now
      list_events_full(calendar, time_min: window_start, time_max: window_end)
    end

    def list_events_full(calendar, time_min:, time_max:)
      range = { start: zoho_time(time_min), end: zoho_time(time_max) }.to_json
      response = connection.get("#{BASE_URL}/calendars/#{calendar.provider_calendar_id}/events", range: range)
      return { events: [], next_sync_token: nil } unless response.success?
      data = JSON.parse(response.body)
      events = Array(data["events"]).map { |e| normalize_event(e) }
      { events: events, next_sync_token: nil }
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
        req.body = { eventdata: build_payload(attrs).to_json, etag: etag }.compact
      end
      raise Calendars::ConflictError, "etag mismatch on #{provider_event_id}" if response.status == 412
      raise_for_status!(response, "update_event")
      parse_single(response)
    end

    def delete_event(calendar, provider_event_id, etag: nil)
      response = connection.delete("#{BASE_URL}/calendars/#{calendar.provider_calendar_id}/events/#{provider_event_id}") do |req|
        req.params["etag"] = etag if etag.present?
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
        color: e["color"].presence, # Zoho stores a hex directly; nil → inherits calendar color
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
      payload[:title] = attrs[:title] if attrs.key?(:title)
      payload[:description] = attrs[:description] if attrs.key?(:description)
      payload[:location] = attrs[:location] if attrs.key?(:location)
      payload[:color] = attrs[:color] if attrs[:color].present? # hex passthrough (Zoho path unverified)
      if attrs[:start_at]
        payload[:dateandtime] = {
          start: zoho_time(attrs[:start_at]),
          end: zoho_time(attrs[:end_at] || (attrs[:start_at] + 3600)),
          timezone: attrs[:time_zone].presence || "UTC"
        }
      end
      payload[:isallday] = true if attrs[:all_day]
      payload
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
        f.request :url_encoded
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Zoho-oauthtoken #{@oauth.access_token}"
      end
    end
  end
end
