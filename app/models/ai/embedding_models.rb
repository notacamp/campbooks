# frozen_string_literal: true

module Ai
  # Catalog of supported embedding models.
  #
  # IMPORTANT: bare `model` values MUST stay unique across entries because DB
  # rows stamp the bare model name (e.g. legacy prod rows are stamped
  # "text-embedding-3-small"), and removing or renaming an entry orphans those
  # stamped rows — the sweeper and search filters would misclassify them.
  module EmbeddingModels
    Entry = Data.define(:key, :provider, :model, :dimensions, :request_dimensions, :max_input_chars, :default) do
      alias_method :default?, :default
    end

    ENTRIES = [
      Entry.new(
        key: "openai/text-embedding-3-small",
        provider: "openai",
        model: "text-embedding-3-small",
        dimensions: 1536,
        request_dimensions: nil,
        max_input_chars: 8000,
        default: true
      ),
      Entry.new(
        key: "openai/text-embedding-3-large",
        provider: "openai",
        model: "text-embedding-3-large",
        dimensions: 3072,
        request_dimensions: nil,
        max_input_chars: 8000,
        default: false
      ),
      Entry.new(
        key: "gemini/gemini-embedding-001",
        provider: "gemini",
        model: "gemini-embedding-001",
        dimensions: 1536,
        request_dimensions: 1536,
        max_input_chars: 6000,
        default: false
      ),
      Entry.new(
        key: "mistral/mistral-embed",
        provider: "mistral",
        model: "mistral-embed",
        dimensions: 1024,
        request_dimensions: nil,
        max_input_chars: 8000,
        default: false
      )
    ].freeze

    DEFAULT = ENTRIES.find(&:default?).freeze

    def self.all
      ENTRIES
    end

    def self.keys
      ENTRIES.map(&:key)
    end

    # Returns the entry for +key+, or nil if not found.
    def self.find(key)
      ENTRIES.find { |e| e.key == key }
    end

    # Returns the entry for +key+, falling back to DEFAULT for nil or unknown
    # keys. Logs a warning when the key is non-nil but unknown (e.g. a stale DB
    # value after a catalog change).
    def self.resolve(key)
      return DEFAULT if key.nil?

      found = find(key)
      unless found
        Rails.logger.warn("[Ai::EmbeddingModels] Unknown embedding model key #{key.inspect}; falling back to DEFAULT")
      end
      found || DEFAULT
    end

    # All unique dimensions values in the catalog, sorted ascending.
    def self.dimensions
      ENTRIES.map(&:dimensions).uniq.sort
    end
  end
end
