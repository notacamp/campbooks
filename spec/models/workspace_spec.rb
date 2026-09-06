# frozen_string_literal: true

require "rails_helper"

RSpec.describe Workspace, type: :model do
  describe "#company_nif" do
    let(:workspace) { described_class.create!(name: "NIF WS") }

    it "returns the explicit company_nif setting when present" do
      workspace.settings["company_nif"] = "PT111222333"
      workspace.settings["company_tax_id"] = "999888777"

      expect(workspace.company_nif).to eq("PT111222333")
    end

    it "falls back to the onboarding company_tax_id when company_nif is unset" do
      workspace.settings["company_tax_id"] = "518692663"

      expect(workspace.company_nif).to eq("518692663")
    end

    it "treats a blank company_nif as unset for the fallback" do
      workspace.settings["company_nif"] = ""
      workspace.settings["company_tax_id"] = "518692663"

      expect(workspace.company_nif).to eq("518692663")
    end

    it "returns nil when neither value exists" do
      expect(workspace.company_nif).to be_nil
    end
  end
  describe "a retired entitlement override" do
    it "no longer fails validation (asks left the paid catalog)" do
      workspace = create(:workspace)
      workspace.update_columns(entitlement_overrides: { "tasks" => { "allowed" => true } })

      expect(workspace.reload).to be_valid
      expect(Entitlements::SchemaComposer.validate_overrides(workspace.entitlement_overrides)).to eq([])
    end

    it "still rejects an unknown feature next to the retired one" do
      workspace = build(:workspace, entitlement_overrides: { "tasks" => { "allowed" => true }, "nonsense" => { "allowed" => true } })

      expect(workspace).not_to be_valid
    end
  end
end
