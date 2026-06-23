module Ai
  module Adapters
    class Openai < Base
      def initialize(api_key:, endpoint_url: nil)
        super
        @endpoint_url = @endpoint_url.presence || DEFAULT_ENDPOINTS["openai"]
      end

      def chat(system:, messages:, model:, max_tokens:, temperature: 0.0)
        openai_messages = build_openai_messages(system, messages)

        body = {
          model: model,
          max_tokens: max_tokens,
          messages: openai_messages
        }
        body[:temperature] = temperature if temperature > 0

        response = connection.post(@endpoint_url) do |req|
          req.body = body.to_json
        end

        extract_text(JSON.parse(response.body))
      rescue Faraday::Error => e
        Rails.logger.error("[OpenAI adapter] HTTP error: #{e.message}")
        raise
      end

      # text-embedding-3-small rejects inputs over 8191 tokens with a 400. A token
      # is always at least one character, so capping each input at this many chars
      # guarantees we stay under the token limit no matter how the text tokenizes
      # — a hard safety net beneath the chunker (chunks are meant to be ~2000
      # tokens, so this only ever trims a mis-sized chunk, never a healthy one).
      EMBED_MAX_INPUT_CHARS = 8000

      def embed(text, model: "text-embedding-3-small")
        input = Array(text).map { |t| t.to_s[0, EMBED_MAX_INPUT_CHARS] }
        embeddings_url = "https://api.openai.com/v1/embeddings"

        response = connection.post(embeddings_url) do |req|
          req.body = { model: model, input: input }.to_json
        end

        data = JSON.parse(response.body)
        vectors = data["data"].sort_by { |d| d["index"] }.map { |d| d["embedding"] }

        text.is_a?(Array) ? vectors : vectors.first
      rescue Faraday::Error => e
        Rails.logger.error("[OpenAI adapter] Embedding error: #{e.message}")
        raise
      end

      private

      def default_headers
        super.merge("Authorization" => "Bearer #{@api_key}")
      end

      def extract_text(body)
        message = body.dig("choices", 0, "message") || {}
        content = message["content"]
        reasoning = message["reasoning_content"]
        finish_reason = body.dig("choices", 0, "finish_reason")

        if content.present?
          return content
        end

        if reasoning.present?
          raise "Reasoning model exhausted max_tokens before producing output (finish_reason=#{finish_reason}, reasoning_length=#{reasoning.length}). Increase max_tokens for this request."
        end

        raise "No content in OpenAI response: #{body.dig("error", "message") || body}"
      end

      def build_openai_messages(system, messages)
        result = []
        result << { role: "system", content: system } if system.present?
        result.concat(translate_messages(messages))
        result
      end

      def translate_messages(messages)
        messages.map { |msg| translate_message(msg) }
      end

      def translate_message(msg)
        if msg[:parts]
          content = msg[:parts].map { |part| translate_part(part) }
          { role: msg[:role] || "user", content: content }
        else
          { role: msg[:role] || "user", content: msg[:content] }
        end
      end

      def translate_part(part)
        case part[:type]
        when :text
          { type: "text", text: part[:text] }
        when :image
          {
            type: "image_url",
            image_url: { url: "data:#{part[:media_type]};base64,#{part[:data]}" }
          }
        when :document
          # OpenAI Chat Completions doesn't support PDF directly.
          # Convert the first page to an image using MiniMagick.
          pdf_to_image(part[:data], part[:media_type])
        else
          { type: "text", text: "" }
        end
      end

      def pdf_to_image(base64_data, media_type)
        return { type: "text", text: "" } unless media_type == "application/pdf"

        Tempfile.create([ "pdf_page", ".pdf" ], binmode: true) do |pdf_file|
          pdf_file.write(Base64.decode64(base64_data))
          pdf_file.rewind

          image = MiniMagick::Image.open(pdf_file.path)
          image.format("jpg")
          image.page("0")  # first page only

          jpeg_data = File.binread(image.path)
          jpeg_base64 = Base64.strict_encode64(jpeg_data)

          {
            type: "image_url",
            image_url: { url: "data:image/jpeg;base64,#{jpeg_base64}" }
          }
        end
      rescue => e
        Rails.logger.warn("[OpenAI adapter] PDF to image conversion failed: #{e.message}")
        { type: "text", text: "[Unsupported document format]" }
      end
    end
  end
end
