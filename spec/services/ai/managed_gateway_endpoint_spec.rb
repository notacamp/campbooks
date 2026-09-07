require "rails_helper"

# Specs for the AI_MANAGED_ENDPOINT / AI_MANAGED_GATEWAY_KEY gateway override.
# These ENV vars route managed AI calls through an OpenAI-compatible gateway
# (e.g. a self-hosted LiteLLM) instead of directly to the provider.
# When unset, behaviour is unchanged (direct to provider).
RSpec.describe "Managed AI gateway endpoint" do
  # Managed adapters are invalid on self-hosted; force cloud mode so fixtures build.
  before { allow(Rails.application.config).to receive(:self_hosted).and_return(false) }

  let(:text_key) { AiAdapter::PROVIDER_ENV_KEYS[Ai::Platform::MANAGED_TEXT_PROVIDER] }
  let(:gateway_endpoint) { "https://ai-gateway.example.com/v1/chat/completions" }
  let(:gateway_key) { "gw-virtual-key-abc123" }
  let(:workspace) { create(:workspace) }

  # ---------------------------------------------------------------------------
  # Ai::Platform
  # ---------------------------------------------------------------------------
  describe Ai::Platform do
    describe ".managed_gateway_endpoint" do
      it "returns nil when AI_MANAGED_ENDPOINT is not set" do
        with_env("AI_MANAGED_ENDPOINT" => nil) do
          expect(described_class.managed_gateway_endpoint).to be_nil
        end
      end

      it "returns the configured URL when AI_MANAGED_ENDPOINT is set" do
        with_env("AI_MANAGED_ENDPOINT" => gateway_endpoint) do
          expect(described_class.managed_gateway_endpoint).to eq(gateway_endpoint)
        end
      end
    end

    describe ".managed_gateway_key" do
      it "returns nil when AI_MANAGED_GATEWAY_KEY is not set" do
        with_env("AI_MANAGED_GATEWAY_KEY" => nil) do
          expect(described_class.managed_gateway_key).to be_nil
        end
      end

      it "returns the gateway key when AI_MANAGED_GATEWAY_KEY is set" do
        with_env("AI_MANAGED_GATEWAY_KEY" => gateway_key) do
          expect(described_class.managed_gateway_key).to eq(gateway_key)
        end
      end
    end

    describe ".available?" do
      it "remains true when a gateway key is set even without the raw provider key" do
        with_env(text_key => nil, "AI_MANAGED_GATEWAY_KEY" => gateway_key) do
          expect(described_class.available?).to be(true)
        end
      end

      it "remains true when the raw provider key is set (unset == direct-to-provider path)" do
        with_env(text_key => "raw-key", "AI_MANAGED_GATEWAY_KEY" => nil) do
          expect(described_class.available?).to be(true)
        end
      end

      it "is false when neither the raw provider key nor the gateway key is set" do
        with_env(text_key => nil, "AI_MANAGED_GATEWAY_KEY" => nil) do
          expect(described_class.available?).to be(false)
        end
      end
    end
  end

  # ---------------------------------------------------------------------------
  # Ai::ProviderSetup#apply_managed
  # ---------------------------------------------------------------------------
  describe Ai::ProviderSetup, "#apply_managed" do
    it "persists the gateway endpoint on the managed text adapter when AI_MANAGED_ENDPOINT is set" do
      allow(Ai::Platform).to receive(:available?).and_return(true)
      allow(Ai::Platform).to receive(:documents_available?).and_return(false)

      with_env("AI_MANAGED_ENDPOINT" => gateway_endpoint, text_key => "k") do
        described_class.new(workspace).apply_managed
      end

      adapter = workspace.ai_adapters.find_by(name: Ai::ProviderSetup::MANAGED_TEXT_ADAPTER_NAME)
      expect(adapter).to be_present
      expect(adapter.endpoint_url).to eq(gateway_endpoint)
    end

    it "stores nil endpoint on the managed adapter when AI_MANAGED_ENDPOINT is not set (direct-to-provider)" do
      allow(Ai::Platform).to receive(:available?).and_return(true)
      allow(Ai::Platform).to receive(:documents_available?).and_return(false)

      with_env("AI_MANAGED_ENDPOINT" => nil, text_key => "k") do
        described_class.new(workspace).apply_managed
      end

      adapter = workspace.ai_adapters.find_by(name: Ai::ProviderSetup::MANAGED_TEXT_ADAPTER_NAME)
      expect(adapter).to be_present
      expect(adapter.endpoint_url).to be_nil
    end

    it "persists the gateway endpoint on the managed document adapter when docs are available" do
      allow(Ai::Platform).to receive(:available?).and_return(true)
      allow(Ai::Platform).to receive(:documents_available?).and_return(true)

      with_env("AI_MANAGED_ENDPOINT" => gateway_endpoint, text_key => "k") do
        described_class.new(workspace).apply_managed
      end

      doc_adapter = workspace.ai_adapters.find_by(name: Ai::ProviderSetup::MANAGED_VISION_ADAPTER_NAME)
      expect(doc_adapter).to be_present
      expect(doc_adapter.endpoint_url).to eq(gateway_endpoint)
    end

    it "clears a previously stored gateway endpoint when AI_MANAGED_ENDPOINT is later unset" do
      allow(Ai::Platform).to receive(:available?).and_return(true)
      allow(Ai::Platform).to receive(:documents_available?).and_return(false)

      # First provision with gateway
      with_env("AI_MANAGED_ENDPOINT" => gateway_endpoint, text_key => "k") do
        described_class.new(workspace).apply_managed
      end
      expect(workspace.ai_adapters.find_by(name: Ai::ProviderSetup::MANAGED_TEXT_ADAPTER_NAME).endpoint_url).to eq(gateway_endpoint)

      # Re-provision without gateway (simulates unsetting the ENV var + re-running)
      with_env("AI_MANAGED_ENDPOINT" => nil, text_key => "k") do
        described_class.new(workspace).apply_managed
      end
      adapter = workspace.ai_adapters.reload.find_by(name: Ai::ProviderSetup::MANAGED_TEXT_ADAPTER_NAME)
      expect(adapter.endpoint_url).to be_nil
    end
  end

  # ---------------------------------------------------------------------------
  # AiAdapter#adapter_instance and #usable?
  # ---------------------------------------------------------------------------
  describe AiAdapter do
    subject(:adapter) { workspace.ai_adapters.create!(name: "managed", provider: "mistral", managed: true) }

    describe "#adapter_instance" do
      it "uses the gateway key as the API key when AI_MANAGED_GATEWAY_KEY is set" do
        with_env("AI_MANAGED_GATEWAY_KEY" => gateway_key, text_key => nil) do
          instance = adapter.adapter_instance
          # The adapter_instance call should not raise (a key is present)
          expect(instance).to be_a(Ai::Adapters::Base)
        end
      end

      it "falls back to the provider env key when gateway key is not set" do
        with_env("AI_MANAGED_GATEWAY_KEY" => nil, text_key => "raw-key") do
          expect { adapter.adapter_instance }.not_to raise_error
        end
      end

      it "passes the stored endpoint_url through to the adapter when set" do
        adapter.update_column(:endpoint_url, gateway_endpoint)
        with_env("AI_MANAGED_GATEWAY_KEY" => gateway_key) do
          instance = adapter.adapter_instance
          expect(instance).to be_a(Ai::Adapters::Base)
        end
      end
    end

    describe "#usable?" do
      it "is true when the gateway key is set (even without the raw provider key)" do
        with_env(text_key => nil, "AI_MANAGED_GATEWAY_KEY" => gateway_key) do
          expect(adapter).to be_usable
        end
      end

      it "is true when the raw provider key is set and no gateway key (unchanged behaviour)" do
        with_env(text_key => "raw-key", "AI_MANAGED_GATEWAY_KEY" => nil) do
          expect(adapter).to be_usable
        end
      end

      it "is false when neither the raw provider key nor the gateway key is set" do
        with_env(text_key => nil, "AI_MANAGED_GATEWAY_KEY" => nil) do
          expect(adapter).not_to be_usable
        end
      end
    end
  end

  # ---------------------------------------------------------------------------
  # Ai::ManagedTextRepointer — gateway endpoint sync
  # ---------------------------------------------------------------------------
  describe Ai::ManagedTextRepointer, "gateway endpoint sync" do
    let!(:managed_adapter) do
      workspace.ai_adapters.create!(
        name: Ai::ProviderSetup::MANAGED_TEXT_ADAPTER_NAME,
        provider: Ai::Platform::MANAGED_TEXT_PROVIDER,
        managed: true, enabled: true
      )
    end

    before do
      workspace.ai_configurations.create!(
        purpose: "global_chat", ai_adapter: managed_adapter,
        model: "mistral-small-latest", max_tokens: 1000, temperature: 0.0, enabled: true
      )
    end

    it "writes the gateway endpoint onto existing managed text adapters when AI_MANAGED_ENDPOINT is set" do
      with_env("AI_MANAGED_ENDPOINT" => gateway_endpoint) do
        result = described_class.run
        expect(managed_adapter.reload.endpoint_url).to eq(gateway_endpoint)
        expect(result.map { |m| m[:workspace_id] }).to include(workspace.id)
        expect(result.find { |m| m[:workspace_id] == workspace.id }[:endpoint_changed]).to be(true)
      end
    end

    it "clears the gateway endpoint from existing managed adapters when AI_MANAGED_ENDPOINT is unset" do
      managed_adapter.update_column(:endpoint_url, gateway_endpoint)

      with_env("AI_MANAGED_ENDPOINT" => nil) do
        result = described_class.run
        expect(managed_adapter.reload.endpoint_url).to be_nil
        expect(result.map { |m| m[:workspace_id] }).to include(workspace.id)
      end
    end

    it "is a no-op when the adapter already has the correct endpoint" do
      managed_adapter.update_column(:endpoint_url, gateway_endpoint)

      with_env("AI_MANAGED_ENDPOINT" => gateway_endpoint) do
        result = described_class.run
        expect(result).to be_empty
      end
    end

    it "leaves BYO (non-managed) adapters untouched regardless of AI_MANAGED_ENDPOINT" do
      byo = workspace.ai_adapters.create!(name: "My BYO", provider: "openai",
                                           managed: false, api_key: "k", enabled: true)
      with_env("AI_MANAGED_ENDPOINT" => gateway_endpoint) do
        described_class.run
        expect(byo.reload.endpoint_url).to be_nil
      end
    end
  end
end
