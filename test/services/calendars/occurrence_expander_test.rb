require "test_helper"

class Calendars::OccurrenceExpanderTest < ActiveSupport::TestCase
  setup { Time.zone = "Europe/Lisbon" }

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
    Calendars::OccurrenceExpander.new(concrete: concrete, masters: masters, from: from, to: to).events
  end

  test "expands a weekly master into the window's occurrences" do
    m = master(rrule: "FREQ=WEEKLY", start_at: Time.zone.local(2026, 7, 6, 9, 0))
    events = expand(concrete: [], masters: [ m ])

    assert_equal [ 6, 13, 20, 27 ], events.map { |e| e.start_at.day }
    assert events.all?(&:occurrence_ghost?), "each expanded row is a ghost"
    assert events.all?(&:recurring?)
    assert_equal 1800, events.first.duration, "duration is preserved"
  end

  test "a real synced instance wins over the ghost of the same slot" do
    m = master(rrule: "FREQ=WEEKLY", start_at: Time.zone.local(2026, 7, 6, 9, 0), pid: "series-1")
    real = synced_instance(pid: "series-1", start_at: Time.zone.local(2026, 7, 13, 9, 0))

    events = expand(concrete: [ real ], masters: [ m ])
    on_13 = events.select { |e| e.start_at.day == 13 }

    assert_equal 1, on_13.size, "7/13 is not drawn twice"
    assert_not on_13.first.occurrence_ghost?, "the concrete instance is kept, not the ghost"
    assert events.detect { |e| e.start_at.day == 6 }.occurrence_ghost?, "other weeks stay ghosts"
  end

  test "dedup tolerates intra-day clock drift (matches by date)" do
    m = master(rrule: "FREQ=WEEKLY", start_at: Time.zone.local(2026, 7, 6, 9, 0), pid: "s")
    # provider materialized the same occurrence an hour off (e.g. a DST shift)
    drifted = synced_instance(pid: "s", start_at: Time.zone.local(2026, 7, 13, 10, 0))

    events = expand(concrete: [ drifted ], masters: [ m ])
    assert_equal 1, events.count { |e| e.start_at.to_date == Date.new(2026, 7, 13) }
  end

  test "keeps concrete non-recurring events and orders everything by start" do
    plain = CalendarEvent.new(title: "One-off", provider_event_id: "p1",
                              start_at: Time.zone.local(2026, 7, 7, 15, 0), end_at: Time.zone.local(2026, 7, 7, 16, 0))
    m = master(rrule: "FREQ=WEEKLY", start_at: Time.zone.local(2026, 7, 6, 9, 0))

    events = expand(concrete: [ plain ], masters: [ m ], from: Time.zone.local(2026, 7, 6), to: Time.zone.local(2026, 7, 8, 23, 59))
    assert_equal [ "Standup", "One-off" ], events.first(2).map(&:title)
  end

  test "a bounded (COUNT) master stops producing occurrences" do
    m = master(rrule: "FREQ=WEEKLY;COUNT=2", start_at: Time.zone.local(2026, 7, 6, 9, 0))
    events = expand(concrete: [], masters: [ m ], to: Time.zone.local(2026, 12, 31))
    assert_equal 2, events.size
  end

  test "a ghost carries the master's id so a click opens the series to edit" do
    m = master(rrule: "FREQ=DAILY", start_at: Time.zone.local(2026, 7, 6, 9, 0))
    m.id = "11111111-1111-1111-1111-111111111111"

    events = expand(concrete: [], masters: [ m ], from: Time.zone.local(2026, 7, 6), to: Time.zone.local(2026, 7, 8, 23, 59))
    assert events.all? { |e| e.id == m.id }
    assert events.none?(&:persisted?), "ghosts are never persisted"
  end

  test "no masters means the concrete list passes through untouched" do
    plain = CalendarEvent.new(title: "Solo", provider_event_id: "p", start_at: Time.zone.local(2026, 7, 7, 9, 0))
    assert_equal [ plain ], expand(concrete: [ plain ], masters: [])
  end
end
