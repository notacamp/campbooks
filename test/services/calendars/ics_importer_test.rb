require "test_helper"

class Calendars::IcsImporterTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  setup do
    ws = Workspace.create!(name: "ICS Importer WS")
    account = ws.calendar_accounts.create!(email_address: "ics@example.com", refresh_token: "tok")
    @calendar = account.calendars.create!(provider_calendar_id: "pc-ics", name: "Primary",
                                          is_writable: true, syncing: true)
    @importer = Calendars::IcsImporter.new(calendar: @calendar)
  end

  test "imports a timed VEVENT shaped like a manual create and enqueues the outbound push" do
    content = ics(vevent([
      "UID:evt-1@example.com",
      "DTSTART:20260710T100000Z",
      "DTEND:20260710T113000Z",
      "SUMMARY:Dentist",
      "DESCRIPTION:Bring the referral",
      "LOCATION:Rua Nova 1"
    ]))

    result = nil
    assert_enqueued_with(job: Calendars::EventWriteJob) do
      result = @importer.call(content)
    end

    assert_equal 1, result.imported
    event = @calendar.calendar_events.find_by(ics_uid: "evt-1@example.com")
    assert_equal "Dentist", event.title
    assert_equal "Bring the referral", event.description
    assert_equal "Rua Nova 1", event.location
    assert_equal Time.utc(2026, 7, 10, 10, 0), event.start_at
    assert_equal Time.utc(2026, 7, 10, 11, 30), event.end_at
    assert_not event.all_day
    assert event.outbound_pending
    assert event.provider_event_id.start_with?("local-")
  end

  test "does not enqueue classification (matches inbound sync behavior)" do
    assert_no_enqueued_jobs(only: EventClassificationJob) do
      @importer.call(ics(vevent([ "UID:e-c", "DTSTART:20260710T100000Z", "SUMMARY:X" ])))
    end
  end

  test "detects all-day events from VALUE=DATE and keeps the exclusive end" do
    content = ics(vevent([
      "UID:evt-allday",
      "DTSTART;VALUE=DATE:20260710",
      "DTEND;VALUE=DATE:20260712",
      "SUMMARY:Offsite"
    ]))

    @importer.call(content)

    event = @calendar.calendar_events.find_by(ics_uid: "evt-allday")
    assert event.all_day
    assert_equal Time.utc(2026, 7, 10), event.start_at
    assert_equal Time.utc(2026, 7, 12), event.end_at
  end

  test "defaults a missing DTEND to one day (all-day) or one hour (timed)" do
    @importer.call(ics(
      vevent([ "UID:no-end-allday", "DTSTART;VALUE=DATE:20260710", "SUMMARY:A" ]),
      vevent([ "UID:no-end-timed", "DTSTART:20260710T100000Z", "SUMMARY:B" ])
    ))

    assert_equal Time.utc(2026, 7, 11), @calendar.calendar_events.find_by(ics_uid: "no-end-allday").end_at
    assert_equal Time.utc(2026, 7, 10, 11), @calendar.calendar_events.find_by(ics_uid: "no-end-timed").end_at
  end

  test "resolves TZID times to UTC" do
    content = ics(vevent([
      "UID:evt-tz",
      "DTSTART;TZID=Europe/Lisbon:20260710T100000",
      "DTEND;TZID=Europe/Lisbon:20260710T110000",
      "SUMMARY:Standup"
    ]))

    @importer.call(content)

    event = @calendar.calendar_events.find_by(ics_uid: "evt-tz")
    assert_equal Time.utc(2026, 7, 10, 9, 0), event.start_at # Lisbon is UTC+1 in July
    assert_equal "Europe/Lisbon", event.start_time_zone
  end

  test "skips recurring VEVENTs and counts them" do
    content = ics(vevent([
      "UID:evt-recurring",
      "DTSTART:20260710T100000Z",
      "RRULE:FREQ=WEEKLY",
      "SUMMARY:Weekly sync"
    ]))

    result = @importer.call(content)

    assert_equal 0, result.imported
    assert_equal 1, result.skipped_recurring
    assert_nil @calendar.calendar_events.find_by(ics_uid: "evt-recurring")
  end

  test "re-importing the same file skips on UID — even after the provider push replaced the temp id" do
    content = ics(vevent([ "UID:evt-dedup", "DTSTART:20260710T100000Z", "SUMMARY:Once" ]))

    assert_equal 1, @importer.call(content).imported
    # Simulate EventWriter#apply_remote! swapping in the provider's real id.
    @calendar.calendar_events.find_by(ics_uid: "evt-dedup")
             .update_columns(provider_event_id: "real_evt_123", outbound_pending: false)

    result = @importer.call(content)

    assert_equal 0, result.imported
    assert_equal 1, result.skipped_duplicate
    assert_equal 1, @calendar.calendar_events.where(ics_uid: "evt-dedup").count
  end

  test "skips VEVENTs without a DTSTART and counts them as malformed" do
    result = @importer.call(ics(vevent([ "UID:evt-broken", "SUMMARY:No time" ])))

    assert_equal 0, result.imported
    assert_equal 1, result.skipped_malformed
  end

  test "caps a file at MAX_EVENTS and flags truncation" do
    events = (1..(Calendars::IcsImporter::MAX_EVENTS + 1)).map do |i|
      vevent([ "UID:bulk-#{i}", "DTSTART:20260710T100000Z", "SUMMARY:Bulk #{i}" ])
    end

    result = @importer.call(ics(*events))

    assert result.truncated
    assert_equal Calendars::IcsImporter::MAX_EVENTS, result.imported
  end

  test "handles garbage and empty input without raising" do
    assert_equal 0, @importer.call("not an ics file").imported
    assert_equal 0, @importer.call("").imported
  end

  private

  def vevent(lines)
    ([ "BEGIN:VEVENT" ] + lines + [ "END:VEVENT" ]).join("\r\n") + "\r\n"
  end

  def ics(*events)
    [ "BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Campbooks Test//EN" ].join("\r\n") +
      "\r\n" + events.join + "END:VCALENDAR\r\n"
  end
end
