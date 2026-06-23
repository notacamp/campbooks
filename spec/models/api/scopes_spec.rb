require "rails_helper"

RSpec.describe Api::Scopes do
  it "stays in sync with the Doorkeeper optional_scopes configuration" do
    configured = Doorkeeper.configuration.optional_scopes.map(&:to_s).sort

    expect(described_class.all.sort).to eq(configured)
  end

  it "provides a human description for every scope" do
    described_class.all.each do |scope|
      expect(described_class.description(scope)).to be_present, "missing description for #{scope}"
    end
  end

  describe ".sanitize" do
    it "keeps only recognized scopes from a mixed array/string input" do
      expect(described_class.sanitize([ "emails:read", "bogus:scope" ])).to eq([ "emails:read" ])
      expect(described_class.sanitize("emails:read documents:read nope")).to contain_exactly("emails:read", "documents:read")
    end
  end
end
