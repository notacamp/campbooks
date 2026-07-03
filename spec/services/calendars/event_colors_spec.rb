require "rails_helper"

RSpec.describe Calendars::EventColors do
  describe ".palette" do
    it "exposes 11 well-spaced colors as { id:, hex:, name: } for the picker" do
      expect(described_class.palette.size).to eq(11)
      expect(described_class.palette.first).to include(:id, :hex, :name)
    end
  end
end
