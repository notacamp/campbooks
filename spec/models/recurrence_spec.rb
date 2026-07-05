require "rails_helper"

RSpec.describe Recurrence do
  before { Time.zone = "Europe/Lisbon" }

  it "blank rrule is non-recurring but valid" do
    r = Recurrence.new(nil)
    expect(r).not_to be_recurring
    expect(r).to be_blank
    expect(r).to be_valid
    expect(r.preset_key).to be_nil
    expect(r.rule).to be_nil
  end

  it "wrap returns the same object or builds one" do
    r = Recurrence.new("FREQ=DAILY")
    expect(Recurrence.wrap(r)).to be(r)
    expect(Recurrence.wrap("FREQ=WEEKLY").rrule).to eq("FREQ=WEEKLY")
  end

  it "strips a leading RRULE: prefix providers send" do
    expect(Recurrence.new("RRULE:FREQ=WEEKLY").rrule).to eq("FREQ=WEEKLY")
  end

  it "every preset parses and maps back to its key" do
    Recurrence::PRESETS.each do |key, rrule|
      r = Recurrence.new(rrule)
      expect(r).to be_valid, "#{rrule} should be valid"
      expect(r).to be_recurring
      expect(r.rule).not_to be_nil, "#{rrule} should parse"
      expect(r.preset_key).to eq(key), "#{rrule} should map back to #{key}"
    end
  end

  it "an arbitrary provider rule is custom, not a preset" do
    r = Recurrence.new("FREQ=WEEKLY;BYDAY=TU;COUNT=5")
    expect(r).to be_recurring
    expect(r.preset_key).to eq(:custom)
    expect(r.rule).not_to be_nil
  end

  it "unparseable rrule is invalid and yields no rule" do
    r = Recurrence.new("this is not a rule")
    expect(r).not_to be_valid
    expect(r.rule).to be_nil
    expect(r.occurrences_between(dtstart: Time.current, from: Time.current, to: 1.year.from_now)).to eq([])
  end

  it "next_occurrence steps daily/weekly/monthly forward from a point" do
    start = Time.zone.local(2026, 7, 6, 9, 0) # Monday

    expect(
      Recurrence.new("FREQ=DAILY").next_occurrence(dtstart: start, after: Time.zone.local(2026, 7, 8, 12, 0))
    ).to eq(Time.zone.local(2026, 7, 9, 9, 0))

    expect(
      Recurrence.new("FREQ=WEEKLY").next_occurrence(dtstart: start, after: Time.zone.local(2026, 7, 14))
    ).to eq(Time.zone.local(2026, 7, 20, 9, 0))

    expect(
      Recurrence.new("FREQ=MONTHLY").next_occurrence(dtstart: start, after: Time.zone.local(2026, 8, 10))
    ).to eq(Time.zone.local(2026, 9, 6, 9, 0))
  end

  it "weekdays preset skips the weekend" do
    start = Time.zone.local(2026, 7, 3, 9, 0) # a Friday
    nxt = Recurrence.preset_rrule(:weekdays)
    r = Recurrence.new(nxt)
    # After Friday, the next weekday occurrence is Monday.
    expect(
      r.next_occurrence(dtstart: start, after: start)
    ).to eq(Time.zone.local(2026, 7, 6, 9, 0))
  end

  it "occurrences_between returns the in-window instances in order" do
    start = Time.zone.local(2026, 7, 6, 9, 0)
    occ = Recurrence.new("FREQ=WEEKLY").occurrences_between(
      dtstart: start, from: Time.zone.local(2026, 7, 1), to: Time.zone.local(2026, 7, 31, 23, 59)
    )
    expect(occ.map { |t| t.day }).to eq([ 6, 13, 20, 27 ])
  end

  it "a bounded rule (COUNT) stops producing occurrences" do
    start = Time.zone.local(2026, 7, 6, 9, 0)
    r = Recurrence.new("FREQ=DAILY;COUNT=3")
    occ = r.occurrences_between(dtstart: start, from: start, to: 1.year.from_now)
    expect(occ.length).to eq(3)
    expect(r.next_occurrence(dtstart: start, after: Time.zone.local(2026, 7, 8, 9, 0))).to be_nil
  end

  it "occurrences_between is capped by limit" do
    start = Time.zone.local(2026, 1, 1, 9, 0)
    occ = Recurrence.new("FREQ=DAILY").occurrences_between(
      dtstart: start, from: start, to: 10.years.from_now, limit: 5
    )
    expect(occ.length).to eq(5)
  end
end
