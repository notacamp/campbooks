require "test_helper"

class RecurrenceTest < ActiveSupport::TestCase
  setup { Time.zone = "Europe/Lisbon" }

  test "blank rrule is non-recurring but valid" do
    r = Recurrence.new(nil)
    assert_not r.recurring?
    assert r.blank?
    assert r.valid?
    assert_nil r.preset_key
    assert_nil r.rule
  end

  test "wrap returns the same object or builds one" do
    r = Recurrence.new("FREQ=DAILY")
    assert_same r, Recurrence.wrap(r)
    assert_equal "FREQ=WEEKLY", Recurrence.wrap("FREQ=WEEKLY").rrule
  end

  test "strips a leading RRULE: prefix providers send" do
    assert_equal "FREQ=WEEKLY", Recurrence.new("RRULE:FREQ=WEEKLY").rrule
  end

  test "every preset parses and maps back to its key" do
    Recurrence::PRESETS.each do |key, rrule|
      r = Recurrence.new(rrule)
      assert r.valid?, "#{rrule} should be valid"
      assert r.recurring?
      assert_not_nil r.rule, "#{rrule} should parse"
      assert_equal key, r.preset_key, "#{rrule} should map back to #{key}"
    end
  end

  test "an arbitrary provider rule is custom, not a preset" do
    r = Recurrence.new("FREQ=WEEKLY;BYDAY=TU;COUNT=5")
    assert r.recurring?
    assert_equal :custom, r.preset_key
    assert_not_nil r.rule
  end

  test "unparseable rrule is invalid and yields no rule" do
    r = Recurrence.new("this is not a rule")
    assert_not r.valid?
    assert_nil r.rule
    assert_equal [], r.occurrences_between(dtstart: Time.current, from: Time.current, to: 1.year.from_now)
  end

  test "next_occurrence steps daily/weekly/monthly forward from a point" do
    start = Time.zone.local(2026, 7, 6, 9, 0) # Monday

    assert_equal Time.zone.local(2026, 7, 9, 9, 0),
      Recurrence.new("FREQ=DAILY").next_occurrence(dtstart: start, after: Time.zone.local(2026, 7, 8, 12, 0))

    assert_equal Time.zone.local(2026, 7, 20, 9, 0),
      Recurrence.new("FREQ=WEEKLY").next_occurrence(dtstart: start, after: Time.zone.local(2026, 7, 14))

    assert_equal Time.zone.local(2026, 9, 6, 9, 0),
      Recurrence.new("FREQ=MONTHLY").next_occurrence(dtstart: start, after: Time.zone.local(2026, 8, 10))
  end

  test "weekdays preset skips the weekend" do
    start = Time.zone.local(2026, 7, 3, 9, 0) # a Friday
    nxt = Recurrence.preset_rrule(:weekdays)
    r = Recurrence.new(nxt)
    # After Friday, the next weekday occurrence is Monday.
    assert_equal Time.zone.local(2026, 7, 6, 9, 0),
      r.next_occurrence(dtstart: start, after: start)
  end

  test "occurrences_between returns the in-window instances in order" do
    start = Time.zone.local(2026, 7, 6, 9, 0)
    occ = Recurrence.new("FREQ=WEEKLY").occurrences_between(
      dtstart: start, from: Time.zone.local(2026, 7, 1), to: Time.zone.local(2026, 7, 31, 23, 59)
    )
    assert_equal [ 6, 13, 20, 27 ], occ.map { |t| t.day }
  end

  test "a bounded rule (COUNT) stops producing occurrences" do
    start = Time.zone.local(2026, 7, 6, 9, 0)
    r = Recurrence.new("FREQ=DAILY;COUNT=3")
    occ = r.occurrences_between(dtstart: start, from: start, to: 1.year.from_now)
    assert_equal 3, occ.length
    assert_nil r.next_occurrence(dtstart: start, after: Time.zone.local(2026, 7, 8, 9, 0))
  end

  test "occurrences_between is capped by limit" do
    start = Time.zone.local(2026, 1, 1, 9, 0)
    occ = Recurrence.new("FREQ=DAILY").occurrences_between(
      dtstart: start, from: start, to: 10.years.from_now, limit: 5
    )
    assert_equal 5, occ.length
  end
end
