require "rails_helper"

RSpec.describe Entitlements::Resolver do
  def resolver_for(plan: "free", overrides: {})
    described_class.new(build(:workspace, plan: plan, entitlement_overrides: overrides))
  end

  describe "#feature?" do
    it "is true for an included feature" do
      expect(resolver_for.feature?(:email_accounts)).to be(true)
    end

    it "is false for a feature the plan does not allow" do
      expect(resolver_for.feature?(:workflows)).to be(false)
    end

    it "is true for managed_ai on every cloud plan (billing is via the deferred quota)" do
      %w[free pro business].each do |plan|
        expect(resolver_for(plan: plan).feature?(:managed_ai)).to be(true)
      end
    end

    it "respects an override that disables a feature" do
      expect(resolver_for(overrides: { "scout" => { "enabled" => false } }).feature?(:scout)).to be(false)
    end

    it "respects an override that grants a feature" do
      r = resolver_for(overrides: { "workflows" => { "allowed" => true, "limit" => 3 } })
      expect(r.feature?(:workflows)).to be(true)
    end
  end

  describe "#limit" do
    it "returns the plan's cap" do
      expect(resolver_for.limit(:email_accounts)).to eq(1)
      expect(resolver_for(plan: "pro").limit(:email_accounts)).to eq(5)
    end

    it "returns nil (unlimited) on business" do
      expect(resolver_for(plan: "business").limit(:email_accounts)).to be_nil
    end

    it "honours an override raising the cap" do
      expect(resolver_for(plan: "pro", overrides: { "email_accounts" => { "limit" => 20 } }).limit(:email_accounts)).to eq(20)
    end
  end

  describe "#config" do
    it "reads a config knob" do
      expect(resolver_for.config(:ai_model_access, :tier)).to eq("basic")
      expect(resolver_for(plan: "business").config(:ai_model_access, :tier)).to eq("premium")
    end
  end

  describe "#allow?" do
    it "is :ok within limit" do
      expect(resolver_for.allow?(:email_accounts)).to eq(:ok)
    end

    it "is :not_allowed for a feature off the plan" do
      expect(resolver_for.allow?(:workflows)).to eq(:not_allowed)
    end

    it "is :ok for a key not in the catalog (never gate the unknown)" do
      expect(resolver_for.allow?(:made_up)).to eq(:ok)
    end
  end

  describe "live usage (against real records)" do
    let(:workspace) { create(:workspace, plan: "free") }
    subject(:resolver) { described_class.new(workspace) }

    before { create(:email_account, workspace: workspace) }

    it "counts active email accounts" do
      expect(resolver.usage(:email_accounts)).to eq(1)
    end

    it "is :over_limit at the cap" do
      expect(resolver.allow?(:email_accounts)).to eq(:over_limit)
    end

    it "reports remaining as zero" do
      expect(resolver.remaining(:email_accounts)).to eq(0)
    end

    it "raises LimitExceeded from allow!" do
      expect { resolver.allow!(:email_accounts) }.to raise_error(Entitlements::LimitExceeded)
    end

    it "flags over_cap after a downgrade leaves it above the cap" do
      create(:email_account, workspace: workspace) # now 2 active, free cap is 1
      expect(resolver.over_cap?(:email_accounts)).to be(true)
    end
  end

  describe "#allow!" do
    it "raises FeatureNotAllowed for a disallowed feature" do
      expect { resolver_for.allow!(:workflows) }.to raise_error(Entitlements::FeatureNotAllowed)
    end
  end

  describe Entitlements::NullResolver do
    subject(:resolver) { described_class.new }

    it "allows everything and caps nothing" do
      expect(resolver.feature?(:workflows)).to be(true)
      expect(resolver.limit(:email_accounts)).to be_nil
      expect(resolver.allow?(:email_accounts)).to eq(:ok)
    end
  end
end
