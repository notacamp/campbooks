# frozen_string_literal: true

module Calendars
  # Parses an .ics file and imports its non-recurring VEVENTs into one writable
  # calendar. Events are built exactly like the manual form's (temp "local-…"
  # provider id, outbound_pending) so each rides the normal EventWriteJob
  # "create" push out to Google/Zoho — the same behavior as Google Calendar's
  # own import. Like inbound sync, no EventClassificationJob is enqueued here
  # (types stay pending; classification is an interactive-create nicety).
  #
  # Dedup: each VEVENT's UID lands in calendar_events.ics_uid (unique per
  # calendar), so re-importing the same file skips cleanly — including after
  # the provider push has replaced the temp provider id. UID-less VEVENTs
  # (rare, spec-violating) can't be deduped and import every time.
  #
  # v1 bounds: recurring VEVENTs are skipped and counted (the outbound write
  # pipeline doesn't push RRULEs), and a file is capped at MAX_EVENTS.
  class IcsImporter
    MAX_EVENTS = 200

    Result = Struct.new(:imported, :skipped_recurring, :skipped_duplicate,
                        :skipped_malformed, :truncated, keyword_init: true)

    def initialize(calendar:)
      @calendar = calendar
    end

    def call(content)
      vevents = Icalendar::Calendar.parse(content.to_s).flat_map(&:events)
      truncated = vevents.size > MAX_EVENTS

      result = Result.new(imported: 0, skipped_recurring: 0, skipped_duplicate: 0,
                          skipped_malformed: 0, truncated: truncated)
      vevents.first(MAX_EVENTS).each { |vevent| import_event(vevent, result) }
      result
    end

    private

    def import_event(vevent, result)
      return result.skipped_recurring += 1 if Array(vevent.rrule).any?

      start_at, end_at, all_day, time_zone = parse_times(vevent)
      return result.skipped_malformed += 1 unless start_at

      uid = vevent.uid.to_s.presence
      return result.skipped_duplicate += 1 if uid && @calendar.calendar_events.exists?(ics_uid: uid)

      event = @calendar.calendar_events.new(
        title: vevent.summary.to_s.strip.presence,
        description: vevent.description.to_s.presence,
        location: vevent.location.to_s.presence,
        start_at: start_at, end_at: end_at, all_day: all_day,
        start_time_zone: time_zone, end_time_zone: time_zone,
        status: :confirmed,
        outbound_pending: true,
        ics_uid: uid,
        provider_event_id: "local-#{SecureRandom.uuid}"
      )

      if event.save
        Calendars::EventWriteJob.perform_later(event.id, "create")
        result.imported += 1
      else
        result.skipped_malformed += 1
      end
    end

    # → [start_at(UTC), end_at(UTC), all_day, tzid]. A VALUE=DATE DTSTART parses
    # as a date (no #hour) ⇒ all-day; DATE-TIME values arrive offset-resolved
    # (icalendar applies VTIMEZONE/TZID via tzinfo). Missing DTEND defaults to
    # +1 day (all-day, exclusive end like the providers) or +1 hour (timed).
    # Icalendar values are delegators, so feature-detect (#hour) rather than
    # is_a?(Date) — the wrapper class defeats kind_of checks.
    def parse_times(vevent)
      dtstart = vevent.dtstart
      return [ nil, nil, false, nil ] unless dtstart

      if dtstart.respond_to?(:hour)
        start_at = dtstart.to_time.utc
        end_at = vevent.dtend ? vevent.dtend.to_time.utc : start_at + 1.hour
        [ start_at, end_at, false, ical_tzid(dtstart) ]
      else
        start_at = dtstart.to_date.to_time(:utc)
        end_date = vevent.dtend ? vevent.dtend.to_date : dtstart.to_date + 1
        [ start_at, end_date.to_time(:utc), true, nil ]
      end
    rescue StandardError
      [ nil, nil, false, nil ]
    end

    def ical_tzid(value)
      return nil unless value.respond_to?(:ical_params)
      Array(value.ical_params["tzid"]).first.to_s.presence
    end
  end
end
