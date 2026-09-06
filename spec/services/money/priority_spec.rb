# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Priority do
  def input(**attrs)
    defaults = { status: :late, days_late: 0, days_until: nil,
                 amount_ratio: nil, counterpart_weight: nil,
                 usual_delay_days: nil, payable: true }
    Money::Priority::Input.new(**defaults.merge(attrs))
  end

  describe ".sat" do
    it "returns 0 when x = 0" do
      expect(described_class.sat(0, 14.0)).to eq(0.0)
    end

    it "approaches 1 as x grows large" do
      expect(described_class.sat(100, 14.0)).to be_within(0.001).of(1.0)
    end
  end

  describe ".urgency" do
    it "returns 0 for settled" do
      expect(described_class.urgency(input(status: :settled))).to eq(0.0)
    end

    it "returns DECIDE_URGENCY for decide" do
      expect(described_class.urgency(input(status: :decide))).to eq(0.6)
    end

    it "is 1 + sat(0, 14) = 1 for late with 0 days" do
      expect(described_class.urgency(input(status: :late, days_late: 0))).to be_within(0.0001).of(1.0)
    end

    it "is > 1 for a late obligation" do
      expect(described_class.urgency(input(status: :late, days_late: 14))).to be > 1.0
    end

    it "is 1.0 for a due obligation due today" do
      expect(described_class.urgency(input(status: :due, days_until: 0))).to be_within(0.0001).of(1.0)
    end

    it "is 0.3 for a due obligation at the 30-day horizon" do
      expect(described_class.urgency(input(status: :due, days_until: 30))).to be_within(0.0001).of(0.3)
    end

    it "clamps days_until to 0..30" do
      expect(described_class.urgency(input(status: :due, days_until: 60))).to be_within(0.0001).of(0.3)
    end
  end

  describe ".size" do
    it "returns 1.0 + sat(0, 2) = 1.0 for nil ratio (treated as 1.0)" do
      expect(described_class.size(input(amount_ratio: nil))).to be_within(0.0001).of(1.0)
    end

    it "is > 1.0 for a ratio of 4" do
      val = described_class.size(input(amount_ratio: 4.0))
      expect(val).to be > 1.0
    end

    it "ignores ratios below 1 (treated as 1)" do
      expect(described_class.size(input(amount_ratio: 0.5))).to eq(described_class.size(input(amount_ratio: 1.0)))
    end
  end

  describe ".who" do
    it "uses the PRIOR when counterpart_weight is nil" do
      expect(described_class.who(input(counterpart_weight: nil))).to eq(0.5 + Attention::Scorer::PRIOR)
    end

    it "returns 0.5 + the given weight" do
      expect(described_class.who(input(counterpart_weight: 0.8))).to be_within(0.0001).of(1.3)
    end
  end

  describe ".habit" do
    it "returns 1.0 for a receivable regardless of delay data" do
      expect(described_class.habit(input(payable: false, status: :late, days_late: 20, usual_delay_days: 0))).to eq(1.0)
    end

    it "returns 1.0 when usual_delay_days is nil" do
      expect(described_class.habit(input(payable: true, status: :late, days_late: 10, usual_delay_days: nil))).to eq(1.0)
    end

    it "returns UNUSUALLY_LATE when days_late > usual + margin" do
      val = described_class.habit(input(payable: true, status: :late, days_late: 12, usual_delay_days: 5))
      expect(val).to eq(1.25)
    end

    it "returns USUAL_LAG when days_late < usual - margin" do
      val = described_class.habit(input(payable: true, status: :late, days_late: 1, usual_delay_days: 8))
      expect(val).to eq(0.8)
    end

    it "returns 1.0 when within the margin band" do
      val = described_class.habit(input(payable: true, status: :late, days_late: 7, usual_delay_days: 5))
      expect(val).to eq(1.0)
    end
  end

  describe ".score" do
    it "returns a Float rounded to 4 decimal places" do
      val = described_class.score(input(status: :late, days_late: 10, counterpart_weight: 0.7))
      expect(val).to be_a(Float)
      expect(val.to_s.split(".").last.length).to be <= 4
    end

    it "late obligation scores higher than due" do
      late  = described_class.score(input(status: :late,  days_late: 5))
      due   = described_class.score(input(status: :due,   days_until: 5))
      expect(late).to be > due
    end

    it "a high counterpart weight raises the score" do
      low  = described_class.score(input(status: :late, days_late: 7, counterpart_weight: 0.1))
      high = described_class.score(input(status: :late, days_late: 7, counterpart_weight: 0.9))
      expect(high).to be > low
    end
  end
end
