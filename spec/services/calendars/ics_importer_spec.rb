require "rails_helper"

RSpec.describe Calendars::IcsImporter do
  let(:ws) { Workspace.create!(name: "ICS Importer WS") }
  let(:account) { ws.calendar_accounts.create!(email_address: "ics@example.com", refresh_token: "tok") }
  let(:calendar) do
    account.calendars.create!(provider_calendar_id: "pc-ics", name: "Primary",
                               is_writable: true, syncing: true)
  end
  let(:importer) { described_class.new(calendar: calendar) }

  it "imports a timed VEVENT shaped like a manual create and enqueues the outbound push" do
    content = ics(vevent([
      "UID:evt-1@example.com",
      "DTSTART:20260710T100000Z",
      "DTEND:20260710T113000Z",
      "SUMMARY:Dentist",
      "DESCRIPTION:Bring the referral",
      "LOCATION:Rua Nova 1"
    ]))

    result = nil
    expect {
      result = importer.call(content)
    }.to have_enqueued_job(Calendars::EventWriteJob)

    expect(result.imported).to eq(1)
    event = calendar.calendar_events.find_by(ics_uid: "evt-1@example.com")
    expect(event.title).to eq("Dentist")
    expect(event.description).to eq("Bring the referral")
    expect(event.location).to eq("Rua Nova 1")
    expect(event.start_at).to eq(Time.utc(2026, 7, 10, 10, 0))
    expect(event.end_at).to eq(Time.utc(2026, 7, 10, 11, 30))
    expect(event.all_day).to be false
    expect(event.outbound_pending).to be true
    expect(event.provider_event_id).to start_with("local-")
  end

  it "does not enqueue classification (matches inbound sync behavior)" do
    expect {
      importer.call(ics(vevent([ "UID:e-c", "DTSTART:20260710T100000Z", "SUMMARY:X" ])))
    }.not_to have_enqueued_job(EventClassificationJob)
  end

  it "detects all-day events from VALUE=DATE and keeps the exclusive end" do
    content = ics(vevent([
      "UID:evt-allday",
      "DTSTART;VALUE=DATE:20260710",
      "DTEND;VALUE=DATE:20260712",
      "SUMMARY:Offsite"
    ]))

    importer.call(content)

    event = calendar.calendar_events.find_by(ics_uid: "evt-allday")
    expect(event.all_day).to be true
    expect(event.start_at).to eq(Time.utc(2026, 7, 10))
    expect(event.end_at).to eq(Time.utc(2026, 7, 12))
  end

  it "defaults a missing DTEND to one day (all-day) or one hour (timed)" do
    importer.call(ics(
      vevent([ "UID:no-end-allday", "DTSTART;VALUE=DATE:20260710", "SUMMARY:A" ]),
      vevent([ "UID:no-end-timed", "DTSTART:20260710T100000Z", "SUMMARY:B" ])
    ))

    expect(calendar.calendar_events.find_by(ics_uid: "no-end-allday").end_at).to eq(Time.utc(2026, 7, 11))
    expect(calendar.calendar_events.find_by(ics_uid: "no-end-timed").end_at).to eq(Time.utc(2026, 7, 10, 11))
  end

  it "resolves TZID times to UTC" do
    content = ics(vevent([
      "UID:evt-tz",
      "DTSTART;TZID=Europe/Lisbon:20260710T100000",
      "DTEND;TZID=Europe/Lisbon:20260710T110000",
      "SUMMARY:Standup"
    ]))

    importer.call(content)

    event = calendar.calendar_events.find_by(ics_uid: "evt-tz")
    expect(event.start_at).to eq(Time.utc(2026, 7, 10, 9, 0)) # Lisbon is UTC+1 in July
    expect(event.start_time_zone).to eq("Europe/Lisbon")
  end

  it "skips recurring VEVENTs and counts them" do
    content = ics(vevent([
      "UID:evt-recurring",
      "DTSTART:20260710T100000Z",
      "RRULE:FREQ=WEEKLY",
      "SUMMARY:Weekly sync"
    ]))

    result = importer.call(content)

    expect(result.imported).to eq(0)
    expect(result.skipped_recurring).to eq(1)
    expect(calendar.calendar_events.find_by(ics_uid: "evt-recurring")).to be_nil
  end

  it "re-importing the same file skips on UID — even after the provider push replaced the temp id" do
    content = ics(vevent([ "UID:evt-dedup", "DTSTART:20260710T100000Z", "SUMMARY:Once" ]))

    expect(importer.call(content).imported).to eq(1)
    # Simulate EventWriter#apply_remote! swapping in the provider's real id.
    calendar.calendar_events.find_by(ics_uid: "evt-dedup")
            .update_columns(provider_event_id: "real_evt_123", outbound_pending: false)

    result = importer.call(content)

    expect(result.imported).to eq(0)
    expect(result.skipped_duplicate).to eq(1)
    expect(calendar.calendar_events.where(ics_uid: "evt-dedup").count).to eq(1)
  end

  it "skips VEVENTs without a DTSTART and counts them as malformed" do
    result = importer.call(ics(vevent([ "UID:evt-broken", "SUMMARY:No time" ])))

    expect(result.imported).to eq(0)
    expect(result.skipped_malformed).to eq(1)
  end

  it "caps a file at MAX_EVENTS and flags truncation" do
    events = (1..(described_class::MAX_EVENTS + 1)).map do |i|
      vevent([ "UID:bulk-#{i}", "DTSTART:20260710T100000Z", "SUMMARY:Bulk #{i}" ])
    end

    result = importer.call(ics(*events))

    expect(result.truncated).to be_truthy
    expect(result.imported).to eq(described_class::MAX_EVENTS)
  end

  it "handles garbage and empty input without raising" do
    expect(importer.call("not an ics file").imported).to eq(0)
    expect(importer.call("").imported).to eq(0)
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
