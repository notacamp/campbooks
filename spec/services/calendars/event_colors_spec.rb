require "rails_helper"

RSpec.describe Calendars::EventColors do
  describe ".palette" do
    it "exposes Google's 11 event colors as { id:, hex:, name: }" do
      expect(described_class.palette.size).to eq(11)
      expect(described_class.palette.first).to include(:id, :hex, :name)
    end
  end

  describe ".hex_for" do
    it "maps a Google colorId (String or Integer) to its hex" do
      expect(described_class.hex_for("5")).to eq("#fbd75b")
      expect(described_class.hex_for(5)).to eq("#fbd75b")
    end

    it "returns nil for a blank or unknown id (event inherits the calendar color)" do
      expect(described_class.hex_for(nil)).to be_nil
      expect(described_class.hex_for("")).to be_nil
      expect(described_class.hex_for("99")).to be_nil
    end
  end

  describe ".id_for" do
    it "maps a palette hex (case-insensitive) back to its colorId" do
      expect(described_class.id_for("#fbd75b")).to eq("5")
      expect(described_class.id_for("#FBD75B")).to eq("5")
    end

    it "returns nil for a blank or non-palette hex (clears to the calendar default)" do
      expect(described_class.id_for(nil)).to be_nil
      expect(described_class.id_for("")).to be_nil
      expect(described_class.id_for("#123456")).to be_nil
    end
  end

  it "round-trips every palette hex through id_for/hex_for" do
    described_class.palette.each do |c|
      expect(described_class.hex_for(described_class.id_for(c[:hex]))).to eq(c[:hex])
    end
  end
end
