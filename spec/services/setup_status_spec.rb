require "rails_helper"

RSpec.describe SetupStatus do
  let(:workspace) { create(:workspace) }

  # Not self-hosted in these examples, so AI readiness comes purely from DB
  # config (no env-key fallback muddying the assertions).
  before { allow(Rails.application.config).to receive(:self_hosted).and_return(false) }

  def configure(purpose, provider: "deepseek")
    adapter = workspace.ai_adapters.find_or_create_by!(name: "adapter-#{provider}") do |a|
      a.provider = provider
      a.api_key = "k"
      a.enabled = true
    end
    workspace.ai_configurations.create!(
      purpose: purpose, ai_adapter: adapter,
      model: "m", max_tokens: 100, temperature: 0.0, enabled: true
    )
  end

  describe "#ai_configuration_complete?" do
    subject { described_class.new(workspace).send(:ai_configuration_complete?) }

    it "is false with no AI configured" do
      expect(subject).to be(false)
    end

    it "is true once a text purpose is set up" do
      configure("email_classification")
      expect(subject).to be(true)
    end

    it "is not satisfied by a document-only (vision) config" do
      configure("document_analysis", provider: "openai")
      expect(subject).to be(false)
    end
  end

  describe "#document_provider_complete?" do
    subject { described_class.new(workspace).send(:document_provider_complete?) }

    it "is independent of the text provider" do
      configure("email_classification")
      expect(subject).to be(false)

      configure("document_analysis", provider: "openai")
      expect(described_class.new(workspace).send(:document_provider_complete?)).to be(true)
    end
  end

  it "exposes the optional document provider as an info-severity item" do
    item = described_class::ITEMS.find { |i| i[:key] == :document_provider }
    expect(item).to be_present
    expect(item[:severity]).to eq(:info)
  end
end
