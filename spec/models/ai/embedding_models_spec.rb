# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::EmbeddingModels do
  describe "ENTRIES" do
    it "has unique keys" do
      keys = described_class::ENTRIES.map(&:key)
      expect(keys.uniq).to eq(keys)
    end

    it "has unique bare model names (DB rows stamp the bare model name)" do
      models = described_class::ENTRIES.map(&:model)
      expect(models.uniq).to eq(models)
    end

    it "has exactly one default entry" do
      defaults = described_class::ENTRIES.select(&:default?)
      expect(defaults.length).to eq(1)
    end

    it "DEFAULT matches the one entry with default: true" do
      expect(described_class::DEFAULT).to eq(described_class::ENTRIES.find(&:default?))
    end

    it "DEFAULT is text-embedding-3-small" do
      expect(described_class::DEFAULT.model).to eq("text-embedding-3-small")
    end

    it "DEFAULT.default? is true" do
      expect(described_class::DEFAULT.default?).to be(true)
    end

    it "non-default entries have default? false" do
      described_class::ENTRIES.reject(&:default?).each do |entry|
        expect(entry.default?).to be(false), "expected #{entry.key} to have default? false"
      end
    end
  end

  describe ".all" do
    it "returns all entries" do
      expect(described_class.all).to eq(described_class::ENTRIES)
    end
  end

  describe ".keys" do
    it "returns all entry keys" do
      expect(described_class.keys).to eq(described_class::ENTRIES.map(&:key))
    end
  end

  describe ".find" do
    it "returns the entry for a known key" do
      entry = described_class.find("openai/text-embedding-3-small")
      expect(entry).not_to be_nil
      expect(entry.model).to eq("text-embedding-3-small")
    end

    it "returns nil for an unknown key" do
      expect(described_class.find("unknown/model")).to be_nil
    end

    it "returns nil for nil" do
      expect(described_class.find(nil)).to be_nil
    end
  end

  describe ".resolve" do
    it "returns the entry for a known key" do
      entry = described_class.resolve("mistral/mistral-embed")
      expect(entry.model).to eq("mistral-embed")
    end

    it "returns DEFAULT for nil" do
      expect(described_class.resolve(nil)).to eq(described_class::DEFAULT)
    end

    it "returns DEFAULT for an unknown key and logs a warning" do
      expect(Rails.logger).to receive(:warn).with(/Unknown embedding model key/)
      result = described_class.resolve("completely/unknown")
      expect(result).to eq(described_class::DEFAULT)
    end

    it "does NOT log a warning for nil" do
      expect(Rails.logger).not_to receive(:warn)
      described_class.resolve(nil)
    end
  end

  describe ".dimensions" do
    it "returns sorted unique dimensions" do
      expect(described_class.dimensions).to eq([1024, 1536, 3072])
    end
  end

  describe "catalog entries have the expected attributes" do
    it "openai/text-embedding-3-small" do
      e = described_class.find("openai/text-embedding-3-small")
      expect(e.provider).to eq("openai")
      expect(e.dimensions).to eq(1536)
      expect(e.request_dimensions).to be_nil
      expect(e.max_input_chars).to eq(8000)
    end

    it "openai/text-embedding-3-large" do
      e = described_class.find("openai/text-embedding-3-large")
      expect(e.provider).to eq("openai")
      expect(e.dimensions).to eq(3072)
      expect(e.request_dimensions).to be_nil
      expect(e.max_input_chars).to eq(8000)
    end

    it "gemini/gemini-embedding-001" do
      e = described_class.find("gemini/gemini-embedding-001")
      expect(e.provider).to eq("gemini")
      expect(e.dimensions).to eq(1536)
      expect(e.request_dimensions).to eq(1536)
      expect(e.max_input_chars).to eq(6000)
    end

    it "mistral/mistral-embed" do
      e = described_class.find("mistral/mistral-embed")
      expect(e.provider).to eq("mistral")
      expect(e.dimensions).to eq(1024)
      expect(e.request_dimensions).to be_nil
      expect(e.max_input_chars).to eq(8000)
    end
  end
end
