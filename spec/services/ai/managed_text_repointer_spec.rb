require "rails_helper"

RSpec.describe Ai::ManagedTextRepointer do
  # Managed adapters are invalid on self-hosted; force cloud mode so the fixtures build.
  before { allow(Rails.application.config).to receive(:self_hosted).and_return(false) }

  def managed_text_adapter(workspace, provider:)
    workspace.ai_adapters.create!(
      name: Ai::ProviderSetup::MANAGED_TEXT_ADAPTER_NAME,
      provider: provider, managed: true, enabled: true
    )
  end

  it "re-points a managed text adapter from deepseek (China) to the EU default and resets the model" do
    ws = create(:workspace)
    adapter = managed_text_adapter(ws, provider: "deepseek")
    config = ws.ai_configurations.create!(purpose: "global_chat", ai_adapter: adapter,
                                          model: "deepseek-v4-pro", max_tokens: 1000, temperature: 0.0, enabled: true)

    result = described_class.run

    expect(adapter.reload.provider).to eq("mistral")
    expect(config.reload.model).to eq("mistral-small-latest")
    expect(result.map { |m| m[:workspace_id] }).to include(ws.id)
  end

  it "leaves BYO (non-managed) adapters untouched" do
    ws = create(:workspace)
    byo = ws.ai_adapters.create!(name: "My DeepSeek", provider: "deepseek", managed: false, api_key: "k", enabled: true)

    described_class.run

    expect(byo.reload.provider).to eq("deepseek")
  end

  it "is idempotent once already on the target provider with valid models" do
    ws = create(:workspace)
    adapter = managed_text_adapter(ws, provider: "mistral")
    ws.ai_configurations.create!(purpose: "global_chat", ai_adapter: adapter,
                                 model: "mistral-small-latest", max_tokens: 1000, temperature: 0.0, enabled: true)

    expect(described_class.run).to eq([])
  end

  # Regression (prod ws#2): a managed adapter already on Mistral but with a config
  # stranded on the old provider's model (deepseek-v4-pro) — Mistral 400s on it.
  # The old repointer skipped already-Mistral adapters, so it never healed this.
  it "normalizes a stale model on an adapter already on the target provider" do
    ws = create(:workspace)
    adapter = managed_text_adapter(ws, provider: "mistral")
    stale = ws.ai_configurations.create!(purpose: "email_classification", ai_adapter: adapter,
                                         model: "deepseek-v4-pro", max_tokens: 1000, temperature: 0.0, enabled: true)
    valid = ws.ai_configurations.create!(purpose: "global_chat", ai_adapter: adapter,
                                         model: "mistral-small-latest", max_tokens: 1000, temperature: 0.0, enabled: true)

    result = described_class.run

    expect(stale.reload.model).to eq("mistral-small-latest")
    expect(valid.reload.model).to eq("mistral-small-latest") # untouched, already valid
    expect(result.map { |m| m[:workspace_id] }).to include(ws.id)
  end
end
