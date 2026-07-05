require "rails_helper"

RSpec.describe Calendars::OccurrenceExpander do
  before { Time.zone = "Europe/Lisbon" }

  def master(rrule:, start_at:, pid: "series-1", **attrs)
    CalendarEvent.new({
      title: "Standup", provider_event_id: pid, rrule: rrule,
      start_at: start_at, end_at: start_at + 1800, all_day: false
    }.merge(attrs))
  end

  def synced_instance(pid:, start_at:)
    CalendarEvent.new(title: "Standup", provider_event_id: "inst-#{start_at.to_i}",
                      recurring_event_provider_id: pid, start_at: start_at, end_at: start_at + 1800)
  end

  def expand(concrete:, masters:, from: Time.zone.local(2026, 7, 1), to: Time.zone.local(2026, 7, 31, 23, 59))
    described_class.new(concrete: concrete, masters: masters, from: from, to: to).events
  end

  it "expands a weekly master into the window's occurrences" do
    m = master(rrule: "FREQ=WEEKLY", start_at: Time.zone.local(2026, 7, 6, 9, 0))
    events = expand(concrete: [], masters: [ m ])

    expect(events.map { |e| e.start_at.day }).to eq([ 6, 13, 20, 27 ])
    expect(events.all?(&:occurrence_ghost?)).to be true # each expanded row is a ghost
    expect(events.all?(&:recurring?)).to be true
    expect(events.first.duration).to eq(1800) # duration is preserved
  end

  it "a real synced instance wins over the ghost of the same slot" do
    m = master(rrule: "FREQ=WEEKLY", start_at: Time.zone.local(2026, 7, 6, 9, 0), pid: "series-1")
    real = synced_instance(pid: "series-1", start_at: Time.zone.local(2026, 7, 13, 9, 0))

    events = expand(concrete: [ real ], masters: [ m ])
    on_13 = events.select { |e| e.start_at.day == 13 }

    expect(on_13.size).to eq(1) # 7/13 is not drawn twice
    expect(on_13.first.occurrence_ghost?).to be_falsey # the concrete instance is kept, not the ghost
    expect(events.detect { |e| e.start_at.day == 6 }.occurrence_ghost?).to be_truthy # other weeks stay ghosts
  end

  it "dedup tolerates intra-day clock drift (matches by date)" do
    m = master(rrule: "FREQ=WEEKLY", start_at: Time.zone.local(2026, 7, 6, 9, 0), pid: "s")
    # provider materialized the same occurrence an hour off (e.g. a DST shift)
    drifted = synced_instance(pid: "s", start_at: Time.zone.local(2026, 7, 13, 10, 0))

    events = expand(concrete: [ drifted ], masters: [ m ])
    expect(events.count { |e| e.start_at.to_date == Date.new(2026, 7, 13) }).to eq(1)
  end

  it "keeps concrete non-recurring events and orders everything by start" do
    plain = CalendarEvent.new(title: "One-off", provider_event_id: "p1",
                              start_at: Time.zone.local(2026, 7, 7, 15, 0), end_at: Time.zone.local(2026, 7, 7, 16, 0))
    m = master(rrule: "FREQ=WEEKLY", start_at: Time.zone.local(2026, 7, 6, 9, 0))

    events = expand(concrete: [ plain ], masters: [ m ], from: Time.zone.local(2026, 7, 6), to: Time.zone.local(2026, 7, 8, 23, 59))
    expect(events.first(2).map(&:title)).to eq([ "Standup", "One-off" ])
  end

  it "a bounded (COUNT) master stops producing occurrences" do
    m = master(rrule: "FREQ=WEEKLY;COUNT=2", start_at: Time.zone.local(2026, 7, 6, 9, 0))
    events = expand(concrete: [], masters: [ m ], to: Time.zone.local(2026, 12, 31))
    expect(events.size).to eq(2)
  end

  it "a ghost carries the master's id so a click opens the series to edit" do
    m = master(rrule: "FREQ=DAILY", start_at: Time.zone.local(2026, 7, 6, 9, 0))
    m.id = "11111111-1111-1111-1111-111111111111"

    events = expand(concrete: [], masters: [ m ], from: Time.zone.local(2026, 7, 6), to: Time.zone.local(2026, 7, 8, 23, 59))
    expect(events.all? { |e| e.id == m.id }).to be true
    expect(events.none?(&:persisted?)).to be true # ghosts are never persisted
  end

  it "no masters means the concrete list passes through untouched" do
    plain = CalendarEvent.new(title: "Solo", provider_event_id: "p", start_at: Time.zone.local(2026, 7, 7, 9, 0))
    expect(expand(concrete: [ plain ], masters: [])).to eq([ plain ])
  end
end
