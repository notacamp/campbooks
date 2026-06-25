require "rails_helper"

RSpec.describe Ai::Provenance do
  describe ".from_config" do
    it "maps the config's provider + model to its data region" do
      expect(described_class.from_config({ provider: "mistral", model: "mistral-small" }))
        .to eq({ "provider" => "mistral", "model" => "mistral-small", "region" => "EU" })
    end

    it "returns {} for a blank config" do
      expect(described_class.from_config(nil)).to eq({})
    end
  end

  describe ".legacy" do
    it "reports the self-hosted Anthropic fallback as US" do
      expect(described_class.legacy("claude-x"))
        .to eq({ "provider" => "anthropic", "model" => "claude-x", "region" => "US" })
    end
  end

  describe ".for_purpose" do
    before { allow(Rails.application.config).to receive(:self_hosted).and_return(false) }

    let(:workspace) { create(:workspace) }

    around do |example|
      Current.workspace = workspace
      example.run
    ensure
      Current.workspace = nil
    end

    it "resolves the provider configured for the purpose" do
      Ai::ProviderSetup.new(workspace).apply_text(provider: "openai", api_key: "byo-key")

      expect(described_class.for_purpose("global_chat", legacy_model: "x"))
        .to include("provider" => "openai", "region" => "US")
    end

    it "returns {} when AI processing is off for the workspace" do
      Ai::ProviderSetup.new(workspace).apply_text(provider: "openai", api_key: "byo-key")
      workspace.update!(ai_processing_enabled: false)

      expect(described_class.for_purpose("global_chat", legacy_model: nil)).to eq({})
    end
  end
end
