module Ai
  module Adapters
    class Base
      DEFAULT_ENDPOINTS = {
        "anthropic" => "https://api.anthropic.com/v1/messages",
        "openai" => "https://api.openai.com/v1/chat/completions",
        "deepseek" => "https://api.deepseek.com/v1/chat/completions",
        "mistral" => "https://api.mistral.ai/v1/chat/completions",
        "gemini" => nil  # URL constructed per-model
      }.freeze

      # Provider hiccups a retry can cure (adapters raise plain Faraday errors via
      # the raise_error middleware). Callers that degrade gracefully on model
      # failure should still let THESE propagate to their job's retry_on —
      # swallowing a rate limit turns a moment's backpressure into lost output.
      TRANSIENT_ERRORS = [
        Faraday::TooManyRequestsError, Faraday::ServerError,
        Faraday::ConnectionFailed, Faraday::TimeoutError
      ].freeze

      def self.for(provider, api_key:, endpoint_url: nil)
        url = endpoint_url.presence
        case provider
        when "anthropic" then Anthropic.new(api_key: api_key, endpoint_url: url)
        when "openai"    then Openai.new(api_key: api_key, endpoint_url: url)
        when "deepseek"  then Openai.new(api_key: api_key, endpoint_url: url || DEFAULT_ENDPOINTS["deepseek"])
        when "mistral"   then Mistral.new(api_key: api_key, endpoint_url: url)
        when "gemini"    then Gemini.new(api_key: api_key, endpoint_url: url)
        else raise ArgumentError, "Unknown AI provider: #{provider}"
        end
      end

      def initialize(api_key:, endpoint_url: nil)
        @api_key = api_key
        @endpoint_url = endpoint_url
      end

      def chat(system:, messages:, model:, max_tokens:, temperature: 0.0)
        raise NotImplementedError
      end

      # Native, tool-aware single turn. Returns an Ai::ChatResult (text +
      # tool_calls + thinking), using the provider's native tool-calling and
      # extended-thinking features rather than a JSON-in-prose protocol.
      #
      # @param tools [Array<Hash>] neutral tool defs: {name:, description:, parameters: <JSON Schema>}
      # @param thinking [Integer, nil] reasoning token budget; nil disables it
      def converse(system:, messages:, model:, max_tokens:, temperature: 0.0, tools: [], thinking: nil)
        raise NotImplementedError
      end

      # Whether this adapter implements native tool calling in `converse`.
      def supports_tools? = false

      # Whether the given model exposes native extended thinking / reasoning.
      def supports_thinking?(_model) = false

      def embed(text, model: "text-embedding-3-small")
        raise NotImplementedError
      end

      private

      def post_json(url, body)
        response = connection.post(url) { |req| req.body = body.to_json }
        JSON.parse(response.body)
      end

      def connection
        @connection ||= Faraday.new(headers: default_headers) do |f|
          f.request :json
          f.response :raise_error
          f.response :logger, Rails.logger, headers: false, bodies: false do |logger|
            logger.filter(/(Authorization: ).*/, '\1[FILTERED]')
            logger.filter(/(x-api-key: ).*/, '\1[FILTERED]')
          end
          f.options.timeout = 120
          f.options.open_timeout = 30
        end
      end

      def default_headers
        { "Content-Type" => "application/json" }
      end

      def extract_text(response_body)
        raise NotImplementedError
      end

      def translate_messages(messages)
        raise NotImplementedError
      end
    end
  end
end
