module Ai
  module Adapters
    class Anthropic < Base
      ANTHROPIC_VERSION = "2024-02-15"

      def initialize(api_key:, endpoint_url: nil)
        super
        @endpoint_url ||= DEFAULT_ENDPOINTS["anthropic"]
      end

      def chat(system:, messages:, model:, max_tokens:, temperature: 0.0)
        body = {
          model: model,
          max_tokens: max_tokens,
          system: system,
          messages: translate_messages(messages)
        }

        # Anthropic doesn't support temperature=0, use small epsilon
        body[:temperature] = temperature > 0 ? temperature : 0.001

        response = connection.post(@endpoint_url) do |req|
          req.body = body.to_json
        end

        extract_text(JSON.parse(response.body))
      rescue Faraday::Error => e
        Rails.logger.error("[Anthropic adapter] HTTP error: #{e.message}")
        raise
      end

      private

      def default_headers
        super.merge(
          "x-api-key" => @api_key,
          "anthropic-version" => ANTHROPIC_VERSION
        )
      end

      def extract_text(body)
        content = body.dig("content")
        text_block = content.find { |c| c["type"] == "text" } if content
        raise "No text content in Anthropic response" unless text_block
        text_block["text"]
      end

      def translate_messages(messages)
        messages.map { |msg| translate_message(msg) }
      end

      def translate_message(msg)
        if msg[:parts]
          # Multi-part message (vision/docs)
          content = msg[:parts].map { |part| translate_part(part) }
          { role: msg[:role] || "user", content: content }
        else
          # Simple text message
          { role: msg[:role] || "user", content: msg[:content] }
        end
      end

      def translate_part(part)
        case part[:type]
        when :text
          { type: "text", text: part[:text] }
        when :image
          {
            type: "image",
            source: { type: "base64", media_type: part[:media_type], data: part[:data] }
          }
        when :document
          {
            type: "document",
            source: { type: "base64", media_type: part[:media_type], data: part[:data] }
          }
        else
          { type: "text", text: "" }
        end
      end
    end
  end
end
