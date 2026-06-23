module Ai
  module Adapters
    # Mistral AI (Paris / EU). The API is OpenAI-compatible (chat completions +
    # message format), so this subclasses the OpenAI adapter and only swaps the
    # endpoint. Having its own class — rather than reusing Openai — makes Mistral a
    # first-class adapter alongside Anthropic/Gemini: selectable per service in
    # Settings → AI, and a home for any Mistral-specific behaviour later.
    #
    # Embeddings stay on OpenAI/Gemini (mistral-embed is 1024-dim vs the 1536-dim
    # search vectors), so #embed is inherited but unused for this provider.
    class Mistral < Openai
      def initialize(api_key:, endpoint_url: nil)
        super(api_key: api_key, endpoint_url: endpoint_url.presence || DEFAULT_ENDPOINTS["mistral"])
      end
    end
  end
end
