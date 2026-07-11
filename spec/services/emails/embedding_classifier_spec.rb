# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::EmbeddingClassifier do
  # A stand-in for a `neighbor`-gem query result: responds to #tag and
  # #neighbor_distance (cosine distance).
  def neighbor(distance, group_name)
    tag = Struct.new(:group_name).new(group_name)
    Struct.new(:neighbor_distance, :tag).new(distance, tag)
  end

  describe ".verdicts_from" do
    it "ranks candidates by similarity, nearest first" do
      list = described_class.verdicts_from([ neighbor(0.40, "Promos"), neighbor(0.10, "Finance"), neighbor(0.25, "Notifications") ])
      expect(list.map(&:group_name)).to eq([ "Finance", "Notifications", "Promos" ])
      expect(list.map { |v| v.similarity.round(2) }).to eq([ 0.90, 0.75, 0.60 ])
    end
  end

  describe ".best_verdict" do
    it "returns nil when there are no neighbors" do
      expect(described_class.best_verdict([])).to be_nil
    end

    it "picks the nearest tag and converts cosine distance to similarity" do
      verdict = described_class.best_verdict([ neighbor(0.30, "Promos"), neighbor(0.10, "Notifications") ])
      expect(verdict.group_name).to eq("Notifications")
      expect(verdict.similarity).to be_within(0.0001).of(0.90)
    end

    it "is confident about a close match and unsure about a far one" do
      close = described_class.best_verdict([ neighbor(0.10, "Finance") ])
      far   = described_class.best_verdict([ neighbor(0.40, "Finance") ])
      expect(close.confident?).to be(true)   # similarity 0.90 >= 0.78
      expect(far.confident?).to be(false)    # similarity 0.60 <  0.78
    end
  end

  # -----------------------------------------------------------------------
  # Model-aware query: stale tag rows are excluded for a switched workspace
  # -----------------------------------------------------------------------
  describe "#verdicts (model-aware)" do
    let(:workspace)     { create(:workspace) }
    let(:email_account) { create(:email_account, workspace: workspace) }
    let(:email_message) { create(:email_message, email_account: email_account) }
    let(:default_entry) { Ai::EmbeddingModels::DEFAULT }
    let(:mistral_entry) { Ai::EmbeddingModels.find("mistral/mistral-embed") }
    let(:tag)           { create(:tag, workspace: workspace, name: "Finance") }

    def vec(dims)
      Array.new(dims) { 0.6 }
    end

    context "when workspace is switched to mistral after default-stamped tags were embedded" do
      before { workspace.update!(embedding_model: "mistral/mistral-embed") }

      it "excludes stale (default-stamped 1536) tag rows from the nearest-neighbor query" do
        # Seed a SearchTagEmbedding that was written before the model switch
        SearchTagEmbedding.create!(
          workspace:       workspace,
          tag:             tag,
          embedding:       vec(1536),
          embedding_model: default_entry.model,
          content_hash:    "old-hash"
        )

        # The fresh_for(mistral_entry) scope must not include the 1536 row
        fresh_ids = SearchTagEmbedding.where(workspace: workspace)
                                     .fresh_for(mistral_entry)
                                     .pluck(:id)

        stale_id = SearchTagEmbedding.find_by!(tag: tag).id
        expect(fresh_ids).not_to include(stale_id)
      end

      it "returns an empty verdict list when there are no mistral-stamped tag embeddings" do
        # No mistral-stamped tags exist; only a stale 1536 row
        SearchTagEmbedding.create!(
          workspace:       workspace,
          tag:             tag,
          embedding:       vec(1536),
          embedding_model: default_entry.model,
          content_hash:    "old"
        )

        allow(EmbeddingService).to receive(:embed).and_return(vec(1024))

        classifier = described_class.new(email_message)
        # Returns empty because the fresh_for filter excludes the stale row
        expect(classifier.shortlist).to be_empty
      end
    end
  end
end
