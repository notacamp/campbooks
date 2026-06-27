require "rails_helper"

RSpec.describe ScheduleCalculator do
  describe ".next_occurrence" do
    it "returns the start time unchanged when it is still in the future" do
      start = 1.day.from_now
      expect(described_class.next_occurrence(start, "FREQ=DAILY")).to be_within(1.second).of(start)
    end

    it "returns nil for a one-time start (no rrule) that has passed" do
      expect(described_class.next_occurrence(1.day.ago, nil)).to be_nil
      expect(described_class.next_occurrence(1.day.ago, "")).to be_nil
    end

    it "advances a daily rule to the next future occurrence, preserving the time of day" do
      start = 3.days.ago.change(hour: 9, min: 0)
      now = Time.current.change(hour: 12)
      nxt = described_class.next_occurrence(start, "FREQ=DAILY", now)

      expect(nxt).to be > now
      expect(nxt.hour).to eq(9)
    end

    it "advances a weekly rule into the future" do
      nxt = described_class.next_occurrence(3.weeks.ago, "FREQ=WEEKLY", Time.current)
      expect(nxt).to be > Time.current
    end

    it "honours INTERVAL for an every-2-weeks rule" do
      nxt = described_class.next_occurrence(5.weeks.ago, "FREQ=WEEKLY;INTERVAL=2", Time.current)
      expect(nxt).to be > Time.current
    end

    it "advances a monthly rule into the future" do
      nxt = described_class.next_occurrence(3.months.ago, "FREQ=MONTHLY", Time.current)
      expect(nxt).to be > Time.current
    end
  end

  describe ".parse_rrule" do
    it "parses key/value pairs case-insensitively" do
      expect(described_class.parse_rrule("FREQ=WEEKLY;INTERVAL=2")).to eq(freq: "WEEKLY", interval: "2")
    end

    it "returns an empty hash for blank input" do
      expect(described_class.parse_rrule("")).to eq({})
    end
  end
end
