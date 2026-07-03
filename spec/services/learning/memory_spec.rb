require "rails_helper"

RSpec.describe Learning::Memory do
  # A minimal in-memory source so the engine can be tested without any DB.
  # `tallies` maps a signal tier to its { label => count } hash (or nil).
  class FakeSource
    def initialize(tallies, on_call: nil)
      @tallies = tallies
      @on_call = on_call
    end

    def signal_cascade = @tallies.keys

    def tally_for(signal, **context)
      @on_call&.call(signal, context)
      @tallies.fetch(signal)
    end

    def min_examples = 3
    def min_share = 0.6
  end

  def build(tallies, **opts)
    described_class.new(source: FakeSource.new(tallies, **opts))
  end

  it "returns the dominant label once the majority and example thresholds are met" do
    s = build({ contact: { "archive" => 3 } }).suggestion

    expect(s.label).to eq("archive")
    expect(s.source).to eq(:contact)
    expect(s.count).to eq(3)
    expect(s.total).to eq(3)
  end

  it "is nil below the minimum number of examples" do
    expect(build({ contact: { "archive" => 2 } }).suggestion).to be_nil
  end

  it "is nil when no single label holds the minimum share" do
    # total 6, top label 3 → 0.5 share, under the 0.6 floor
    expect(build({ contact: { "archive" => 3, "keep" => 3 } }).suggestion).to be_nil
  end

  it "accepts a label sitting exactly on the share threshold" do
    # total 5, top label 3 → exactly 0.6
    expect(build({ contact: { "archive" => 3, "keep" => 2 } }).suggestion.label).to eq("archive")
  end

  it "walks the cascade in order and reports the winning tier" do
    s = build({ contact: nil, domain: { "keep" => 4 } }).suggestion

    expect(s.source).to eq(:domain)
    expect(s.label).to eq("keep")
  end

  it "short-circuits: a later tier is never consulted once an earlier one wins" do
    consulted = []
    build({ contact: { "archive" => 3 }, domain: { "keep" => 9 } },
          on_call: ->(signal, _) { consulted << signal }).suggestion

    expect(consulted).to eq(%i[contact])
  end

  it "forwards caller context to the source (bulk-lookup keys)" do
    seen = nil
    build({ contact: { "archive" => 3 } },
          on_call: ->(_, context) { seen = context }).suggestion(contact: "c-1", domain: "x.com")

    expect(seen).to eq(contact: "c-1", domain: "x.com")
  end

  it "is nil when every tier is empty" do
    expect(build({ contact: nil, domain: {} }).suggestion).to be_nil
  end
end
