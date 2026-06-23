module Ai
  module Adapters
    class Gemini < Base
      BASE_URL = "https://generativelanguage.googleapis.com/v1beta"

      def chat(system:, messages:, model:, max_tokens:, temperature: 0.0)
        url = "#{BASE_URL}/models/#{model}:generateContent?key=#{@api_key}"

        contents = translate_messages(messages)
        body = {
          contents: contents,
          generationConfig: {
            maxOutputTokens: max_tokens
          }
        }
        body[:generationConfig][:temperature] = temperature if temperature > 0

        if system.present?
          body[:systemInstruction] = { parts: [ { text: system } ] }
        end

        response = connection.post(url) do |req|
          req.body = body.to_json
        end

        extract_text(JSON.parse(response.body))
      rescue Faraday::Error => e
        Rails.logger.error("[Gemini adapter] HTTP error: #{e.message}")
        raise
      end

      def embed(text, model: "text-embedding-3-small")
        input = Array(text)
        embeddings_url = "https://generativelanguage.googleapis.com/v1beta/openai/embeddings"

        response = connection.post(embeddings_url) do |req|
          req.headers["Authorization"] = "Bearer #{@api_key}"
          req.body = { model: model, input: input }.to_json
        end

        data = JSON.parse(response.body)
        vectors = data["data"].sort_by { |d| d["index"] }.map { |d| d["embedding"] }

        text.is_a?(Array) ? vectors : vectors.first
      rescue Faraday::Error => e
        Rails.logger.error("[Gemini adapter] Embedding error: #{e.message}")
        raise
      end

      private

      def extract_text(body)
        body.dig("candidates", 0, "content", "parts", 0, "text") ||
          raise("No text content in Gemini response: #{body.dig("error", "message")}")
      end

      def translate_messages(messages)
        messages.map { |msg| translate_message(msg) }
      end

      def translate_message(msg)
        if msg[:parts]
          parts = msg[:parts].map { |part| translate_part(part) }
          { role: msg[:role] == "assistant" ? "model" : "user", parts: parts }
        else
          role = msg[:role] == "assistant" ? "model" : "user"
          { role: role, parts: [ { text: msg[:content] } ] }
        end
      end

      def translate_part(part)
        case part[:type]
        when :text
          { text: part[:text] }
        when :image, :document
          {
            inlineData: {
              mimeType: part[:media_type],
              data: part[:data]
            }
          }
        else
          { text: "" }
        end
      end
    end
  end
end
