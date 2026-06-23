require "rails_helper"

# Mistral (Paris / EU) is added as an OpenAI-compatible provider and made the
# managed text default, so a cloud workspace's email + chat content stays in the
# EU by default instead of going to DeepSeek (China). See task #8 / docs/gdpr.
RSpec.describe "Mistral AI provider" do
  it "is a selectable provider" do
    expect(AiConfiguration::PROVIDERS).to include("mistral")
    expect(AiAdapter::PROVIDERS).to include("mistral")
  end

  it "has a default model, model list, and env key" do
    expect(AiConfiguration::DEFAULT_MODEL["mistral"]).to eq("mistral-small-latest")
    expect(AiConfiguration::MODELS["mistral"]).to include("mistral-large-latest")
    expect(AiConfiguration::PROVIDER_ENV_KEYS["mistral"]).to eq("MISTRAL_API_KEY")
    expect(AiAdapter::PROVIDER_ENV_KEYS["mistral"]).to eq("MISTRAL_API_KEY")
  end

  it "records a data region for every provider (mistral = EU)" do
    expect(AiConfiguration::PROVIDER_REGIONS["mistral"]).to eq("EU")
    AiConfiguration::PROVIDERS.each do |provider|
      expect(AiConfiguration::PROVIDER_REGIONS).to have_key(provider), "missing region for #{provider}"
    end
  end

  it "builds an OpenAI-compatible adapter pointed at Mistral's EU endpoint" do
    expect(Ai::Adapters::Base::DEFAULT_ENDPOINTS["mistral"]).to eq("https://api.mistral.ai/v1/chat/completions")
    expect(Ai::Adapters::Base.for("mistral", api_key: "k")).to be_a(Ai::Adapters::Mistral)
    expect(Ai::Adapters::Base.for("mistral", api_key: "k")).to be_a(Ai::Adapters::Openai) # OpenAI-compatible
  end

  it "is the managed text default (EU), replacing DeepSeek/China" do
    expect(Ai::Platform::MANAGED_TEXT_PROVIDER).to eq("mistral")
    expect(Ai::Platform.text_model).to eq("mistral-small-latest")
    expect(AiConfiguration::PROVIDER_REGIONS[Ai::Platform::MANAGED_TEXT_PROVIDER]).to eq("EU")
  end

  it "is accepted as a valid adapter provider" do
    workspace = create(:workspace)
    adapter = workspace.ai_adapters.build(name: "Mistral EU", provider: "mistral", api_key: "k")
    expect(adapter).to be_valid
  end
end
