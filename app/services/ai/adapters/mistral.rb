# frozen_string_literal: true

module Ai
  module Adapters
    # Mistral AI (Paris / EU). The API is OpenAI-compatible (chat completions +
    # message format), so this subclasses the OpenAI adapter and only swaps the
    # chat endpoint and the embeddings endpoint.
    #
    # mistral-embed is 1024-dim and is now a first-class catalog entry — EU
    # workspaces that need data-residency can use it for semantic search.
    # The `dimensions` parameter is unused by mistral-embed (the catalog sets
    # request_dimensions: nil), so Openai#embed's `if dimensions` guard means
    # it is never sent — no extra code needed here.
    class Mistral < Openai
      def initialize(api_key:, endpoint_url: nil)
        super(api_key: api_key, endpoint_url: endpoint_url.presence || DEFAULT_ENDPOINTS["mistral"])
      end

      private

      # mistral-embed lives at the dedicated embeddings endpoint, not under /chat/completions.
      def embeddings_endpoint
        "https://api.mistral.ai/v1/embeddings"
      end
    end
  end
end
