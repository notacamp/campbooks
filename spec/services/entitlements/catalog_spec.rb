require "rails_helper"

RSpec.describe Entitlements::Catalog do
  subject(:catalog) { described_class.instance }

  it "loads every plan without error" do
    expect(catalog.plan_names).to include("free", "pro", "business", "unlimited")
  end

  it "exposes the expected feature keys" do
    expect(catalog.feature_keys).to include(:email_accounts, :workflows, :managed_ai, :scout, :ai_model_access)
  end

  it "builds a non-empty spec set for each plan" do
    catalog.plan_names.each do |name|
      expect(catalog.plan(name)).not_to be_empty, "plan #{name} is empty"
    end
  end

  it "falls back to the default plan for an unknown name" do
    expect(catalog.plan("does-not-exist")).to eq(catalog.plan(Entitlements::Catalog::DEFAULT_PLAN))
  end

  describe Entitlements::SchemaComposer do
    let(:schema) { described_class.build }

    it "produces a JSON schema keyed by feature" do
      expect(schema["properties"].keys).to include("email_accounts", "workflows")
    end

    it "accepts a well-formed override" do
      expect(described_class.validate_overrides({ "email_accounts" => { "limit" => 5 } })).to be_empty
    end

    it "rejects an unknown feature key" do
      expect(described_class.validate_overrides({ "bogus" => {} })).not_to be_empty
    end

    it "rejects a wrongly-typed limit" do
      expect(described_class.validate_overrides({ "email_accounts" => { "limit" => "lots" } })).not_to be_empty
    end
  end
end
