# frozen_string_literal: true

require "rails_helper"

RSpec.describe Time::SlotFinder do
  let(:zone) { ActiveSupport::TimeZone["Europe/Lisbon"] }

  def find(from:, to:, busy: [], duration: 45)
    described_class.find(from: from, to: to, duration_minutes: duration, busy: busy, zone: zone)
  end

  it "prefers 10:00 on the first working day of the window" do
    monday = zone.local(2026, 9, 7, 9, 0) # a Monday
    expect(find(from: monday, to: monday + 5.days)).to eq(zone.local(2026, 9, 7, 10, 0))
  end

  it "skips Saturdays and Sundays" do
    saturday = zone.local(2026, 9, 5, 9, 0)
    expect(find(from: saturday, to: saturday + 5.days)).to eq(zone.local(2026, 9, 7, 10, 0))
  end

  it "returns nil when the window holds only weekend days" do
    saturday = zone.local(2026, 9, 5, 9, 0)
    expect(find(from: saturday, to: saturday + 1.day + 9.hours)).to be_nil
  end

  it "takes the earliest free grid slot when 10:00 is busy, avoiding the busy interval" do
    monday = zone.local(2026, 9, 7, 9, 0)
    busy = [ [ zone.local(2026, 9, 7, 9, 0), zone.local(2026, 9, 7, 11, 0) ] ]
    expect(find(from: monday, to: monday + 1.day, busy: busy)).to eq(zone.local(2026, 9, 7, 11, 0))
  end

  it "clamps to working hours: nothing after 18:00, so the next working day is used" do
    monday_evening = zone.local(2026, 9, 7, 17, 30)
    expect(find(from: monday_evening, to: monday_evening + 2.days)).to eq(zone.local(2026, 9, 8, 10, 0))
  end
end
