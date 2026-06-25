require "rails_helper"

# EU data-residency enforcement: a workspace with required_data_region = "EU" may
# only USE EU-region AI providers. Text AI (Mistral/EU) continues; document AI and
# embeddings (US providers) pause.
RSpec.describe "AI data residency", type: :model do
  before { allow(Rails.application.config).to receive(:self_hosted).and_return(false) }

  let(:workspace) { create(:workspace) }

  describe "Workspace#region_allows?" do
    it "allows any provider when no policy is set" do
      expect(workspace.region_allows?("openai")).to be(true)
      expect(workspace.region_allows?("mistral")).to be(true)
    end

    it "allows only EU-region providers under an EU policy" do
      workspace.update!(required_data_region: "EU")
      expect(workspace.region_allows?("mistral")).to be(true)   # EU
      expect(workspace.region_allows?("openai")).to be(false)   # US
      expect(workspace.region_allows?("deepseek")).to be(false) # China
    end
  end

  describe "Ai::Configuration.for" do
    around do |example|
      Current.workspace = workspace
      example.run
    ensure
      Current.workspace = nil
    end

    it "pauses a purpose whose provider violates the EU policy" do
      Ai::ProviderSetup.new(workspace).apply_text(provider: "openai", api_key: "byo-key")
      purpose = AiConfiguration::TEXT_PURPOSES.first

      expect(Ai::Configuration.for(purpose)).to be_present # no policy yet

      workspace.update!(required_data_region: "EU")
      expect(Ai::Configuration.for(purpose)).to be_nil # US provider paused
    end

    it "still resolves an EU provider under the EU policy" do
      Ai::ProviderSetup.new(workspace).apply_text(provider: "mistral", api_key: "byo-key")
      workspace.update!(required_data_region: "EU")

      config = Ai::Configuration.for(AiConfiguration::TEXT_PURPOSES.first)
      expect(config).to be_present
      expect(config[:provider]).to eq("mistral")
    end
  end

  describe "EmbeddingService" do
    it "resolves no embedding adapter under an EU policy (OpenAI/Gemini are US)" do
      Ai::ProviderSetup.new(workspace).apply_text(provider: "openai", api_key: "byo-key")
      workspace.update!(required_data_region: "EU")

      expect(EmbeddingService.new(workspace).send(:find_embedding_adapter)).to be_nil
    end
  end
end
