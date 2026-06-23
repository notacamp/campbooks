require "rails_helper"

RSpec.describe Workspace, "entitlements", type: :model do
  describe "validations" do
    it "rejects an unknown plan" do
      ws = build(:workspace, plan: "enterprise")
      expect(ws).not_to be_valid
      expect(ws.errors[:plan]).to be_present
    end

    it "accepts a known plan" do
      expect(build(:workspace, plan: "pro")).to be_valid
    end

    it "rejects entitlement_overrides with an unknown feature key" do
      ws = build(:workspace, entitlement_overrides: { "bogus" => { "enabled" => true } })
      expect(ws).not_to be_valid
      expect(ws.errors[:entitlement_overrides]).to be_present
    end

    it "accepts well-formed entitlement_overrides" do
      expect(build(:workspace, entitlement_overrides: { "email_accounts" => { "limit" => 9 } })).to be_valid
    end
  end

  describe "#entitlements" do
    it "returns a Resolver on cloud" do
      expect(build(:workspace).entitlements).to be_a(Entitlements::Resolver)
    end

    it "returns a NullResolver on self-hosted installs" do
      allow(Rails.application.config).to receive(:self_hosted).and_return(true)
      expect(build(:workspace).entitlements).to be_a(Entitlements::NullResolver)
    end
  end
end
