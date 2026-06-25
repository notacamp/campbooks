require "rails_helper"

# The global AI kill-switch (Settings → Data & Privacy → ai_processing_enabled).
# When off, every AI surface must fail closed at the three central gates.
RSpec.describe "Global AI kill-switch" do
  before { allow(Rails.application.config).to receive(:self_hosted).and_return(false) }

  let(:workspace) { create(:workspace) }

  # A working BYO text provider so the only thing flipping the gates is the switch.
  # An OpenAI adapter also satisfies the embeddings capability.
  before { Ai::ProviderSetup.new(workspace).apply_text(provider: "openai", api_key: "byo-key") }

  describe "Ai::ProviderSetup" do
    it "reports AI working while enabled (the default)" do
      setup = Ai::ProviderSetup.new(workspace)
      expect(setup.available?(:text)).to be(true)
      expect(setup.configured?(:text)).to be(true)
      expect(setup.configured?(:embeddings)).to be(true)
    end

    it "fails closed for every capability once the workspace turns AI off" do
      workspace.update!(ai_processing_enabled: false)
      setup = Ai::ProviderSetup.new(workspace)

      %i[text documents embeddings].each do |cap|
        expect(setup.available?(cap)).to be(false), "available?(#{cap}) should be false when AI is off"
        expect(setup.configured?(cap)).to be(false), "configured?(#{cap}) should be false when AI is off"
      end
    end
  end

  describe "Ai::Configuration.for" do
    around do |example|
      Current.workspace = workspace
      example.run
    ensure
      Current.workspace = nil
    end

    it "resolves a provider while AI is enabled" do
      expect(Ai::Configuration.for("global_chat")).to be_present
    end

    it "returns nil for a configured purpose when AI is off" do
      workspace.update!(ai_processing_enabled: false)
      expect(Ai::Configuration.for("global_chat")).to be_nil
    end
  end

  describe "EmbeddingService#embed_batch" do
    it "returns [] without resolving a provider when AI is off" do
      workspace.update!(ai_processing_enabled: false)
      service = EmbeddingService.new(workspace)
      expect(service).not_to receive(:find_embedding_adapter)
      expect(service.embed_batch([ "hello" ])).to eq([])
    end
  end
end
