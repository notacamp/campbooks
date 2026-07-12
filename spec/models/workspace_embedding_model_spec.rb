# frozen_string_literal: true

require "rails_helper"

RSpec.describe Workspace, type: :model do
  describe "embedding_model validation" do
    it "is valid with nil (use default)" do
      ws = build(:workspace, embedding_model: nil)
      expect(ws).to be_valid
    end

    it "is valid with a known catalog key" do
      ws = build(:workspace, embedding_model: "openai/text-embedding-3-small")
      expect(ws).to be_valid
    end

    it "is valid with mistral entry key" do
      ws = build(:workspace, embedding_model: "mistral/mistral-embed")
      expect(ws).to be_valid
    end

    it "is invalid with an unknown key" do
      ws = build(:workspace, embedding_model: "unknown/model")
      expect(ws).not_to be_valid
      expect(ws.errors[:embedding_model]).not_to be_empty
    end
  end

  describe "#embedding_model_entry" do
    it "returns DEFAULT when embedding_model is nil" do
      ws = build(:workspace, embedding_model: nil)
      expect(ws.embedding_model_entry).to eq(Ai::EmbeddingModels::DEFAULT)
    end

    it "returns the matching entry when embedding_model is set" do
      ws = build(:workspace, embedding_model: "mistral/mistral-embed")
      entry = ws.embedding_model_entry
      expect(entry.model).to eq("mistral-embed")
      expect(entry.dimensions).to eq(1024)
    end

    it "returns DEFAULT for an unknown key (resolve fallback)" do
      ws = build(:workspace)
      allow(ws).to receive(:embedding_model).and_return("stale/unknown")
      allow(Rails.logger).to receive(:warn)
      expect(ws.embedding_model_entry).to eq(Ai::EmbeddingModels::DEFAULT)
    end
  end
end
