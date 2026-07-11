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

      # NOTE: the previous implementation called Gemini's OpenAI-compatibility
      # endpoint (/v1beta/openai/embeddings) while passing an OpenAI model name like
      # "text-embedding-3-small". That endpoint only accepts Gemini model IDs, so it
      # could never work for real Gemini embedding models. We now use the native
      # batchEmbedContents API instead.
      def embed(text, model:, dimensions: nil)
        input = Array(text)
        url = "#{BASE_URL}/models/#{model}:batchEmbedContents?key=#{@api_key}"

        requests = input.map do |t|
          req = {
            model: "models/#{model}",
            content: { parts: [ { text: t } ] }
          }
          req[:outputDimensionality] = dimensions if dimensions
          req
        end

        response = connection.post(url) do |req|
          req.body = { requests: requests }.to_json
        end

        data = JSON.parse(response.body)
        vectors = data["embeddings"].map { |e| e["values"] }

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
