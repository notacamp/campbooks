# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::DocumentProviderRouter do
  # BYO path: not self-hosted, so the routed adapter stores the platform key.
  before { allow(Rails.application.config).to receive(:self_hosted).and_return(false) }

  # The Anthropic key the router needs to be present.
  before do
    allow(ENV).to receive(:[]).and_call_original
    allow(ENV).to receive(:[]).with("ANTHROPIC_API_KEY").and_return("test-anthropic-key")
  end

  def doc_config(ws)
    ws.ai_configurations.find_by(purpose: "document_analysis")
  end

  describe ".run" do
    it "re-points an OpenAI document workspace onto a DEDICATED Claude adapter" do
      ws = create(:workspace)
      Ai::ProviderSetup.new(ws).apply_documents(provider: "openai", api_key: "openai-key")
      original_adapter_id = doc_config(ws).ai_adapter_id

      result = described_class.run

      cfg = doc_config(ws).reload
      expect(cfg.ai_adapter.provider).to eq("anthropic")
      expect(cfg.ai_adapter.name).to eq("Document AI provider (Claude)")
      expect(cfg.model).to eq("claude-sonnet-4-6")
      # The original OpenAI adapter is left intact (it may drive compose_chat / embeddings).
      expect(AiAdapter.exists?(original_adapter_id)).to be(true)
      expect(cfg.ai_adapter_id).not_to eq(original_adapter_id)
      expect(result).to include(hash_including(workspace_id: ws.id, from: "openai", to: "anthropic"))
    end

    it "skips a workspace that never configured a document provider" do
      ws = create(:workspace)

      result = described_class.run(only_workspace_id: ws.id)

      expect(doc_config(ws)).to be_nil
      expect(result).to eq([ { workspace_id: ws.id, skipped: "no document provider configured" } ])
    end

    it "skips a workspace already on a usable Anthropic adapter" do
      ws = create(:workspace)
      Ai::ProviderSetup.new(ws).apply_documents(provider: "anthropic", api_key: "stored-key")

      result = described_class.run(only_workspace_id: ws.id)

      expect(result.first[:skipped]).to eq("already on Anthropic")
    end

    it "does not persist anything in dry-run mode" do
      ws = create(:workspace)
      Ai::ProviderSetup.new(ws).apply_documents(provider: "openai", api_key: "openai-key")

      described_class.run(dry_run: true, only_workspace_id: ws.id)

      expect(doc_config(ws).reload.ai_adapter.provider).to eq("openai")
    end

    it "skips when no Anthropic key is available" do
      allow(ENV).to receive(:[]).with("ANTHROPIC_API_KEY").and_return(nil)
      ws = create(:workspace)
      Ai::ProviderSetup.new(ws).apply_documents(provider: "openai", api_key: "openai-key")

      result = described_class.run(only_workspace_id: ws.id)

      expect(result.first[:skipped]).to eq("no ANTHROPIC_API_KEY available")
    end
  end
end
