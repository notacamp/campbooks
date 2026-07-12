# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmbedTagJob, type: :job do
  let(:workspace)     { create(:workspace) }
  let(:default_entry) { Ai::EmbeddingModels::DEFAULT }
  let(:mistral_entry) { Ai::EmbeddingModels.find("mistral/mistral-embed") }
  let(:tag)           { create(:tag, workspace: workspace, name: "Finance") }

  def vec(dims)
    Array.new(dims) { 0.3 }
  end

  before do
    # Gate passes by default
    allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)
  end

  describe "model switch re-embeds even when content hash is unchanged" do
    it "re-embeds into embedding_1024 when workspace switches from default to mistral" do
      workspace.update!(embedding_model: nil)  # default = openai

      # Seed an existing embedding stamped for the default model
      content      = SearchTagEmbedding.embedding_text_for(tag)
      content_hash = Digest::SHA256.hexdigest(content)
      existing = SearchTagEmbedding.create!(
        workspace:       workspace,
        tag:             tag,
        embedding:       vec(1536),
        embedding_model: default_entry.model,
        content_hash:    content_hash
      )

      # Now switch the workspace to mistral
      workspace.update!(embedding_model: "mistral/mistral-embed")

      # Job should re-embed even though the content hash is unchanged
      allow(EmbeddingService).to receive(:embed).and_return(vec(1024))

      described_class.perform_now(tag)

      existing.reload
      expect(existing.embedding_model).to eq("mistral-embed")
      expect(existing.embedding_1024).to be_present
      expect(existing.embedding).to be_nil  # cleared by assign_embedding
    end
  end

  describe "unchanged content hash + same model → skipped" do
    it "does not call EmbeddingService when content and model are both unchanged" do
      content      = SearchTagEmbedding.embedding_text_for(tag)
      content_hash = Digest::SHA256.hexdigest(content)
      SearchTagEmbedding.create!(
        workspace:       workspace,
        tag:             tag,
        embedding:       vec(1536),
        embedding_model: default_entry.model,
        content_hash:    content_hash
      )

      expect(EmbeddingService).not_to receive(:embed)
      described_class.perform_now(tag)
    end
  end
end
