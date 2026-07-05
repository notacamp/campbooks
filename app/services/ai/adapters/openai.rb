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
          req.options.context = { model: model.to_s }
        end

        extract_text(JSON.parse(response.body))
      rescue Faraday::Error => e
        Rails.logger.error("[OpenAI adapter] HTTP error: #{e.message}")
        raise
      end

      # Model-name fragments that identify a reasoning model (exposes a thinking
      # trace via reasoning_content and needs max_completion_tokens, no temperature).
      REASONING_MODELS = %w[reasoner o1 o3 o4-mini gpt-5].freeze

      # DeepSeek reuses this adapter pointed at api.deepseek.com. Report the
      # correct service key so the health dashboard distinguishes the two.
      def system_health_service
        @endpoint_url.to_s.include?("deepseek") ? "ai_deepseek" : super
      end

      def supports_tools? = true

      def supports_thinking?(model)
        REASONING_MODELS.any? { |frag| model.to_s.include?(frag) }
      end

      def converse(system:, messages:, model:, max_tokens:, temperature: 0.0, tools: [], thinking: nil)
        reasoning = supports_thinking?(model)

        body = { model: model, messages: build_openai_messages(system, messages) }
        # Reasoning models reject `max_tokens`/`temperature`; everything else needs them.
        if reasoning
          body[:max_completion_tokens] = max_tokens
          body[:reasoning_effort] = reasoning_effort(thinking) if thinking
        else
          body[:max_tokens] = max_tokens
          body[:temperature] = temperature if temperature > 0
        end
        if tools.any?
          body[:tools] = tools.map { |t| { type: "function", function: t } }
          body[:tool_choice] = "auto"
        end

        parse_converse(post_json(@endpoint_url, body))
      rescue Faraday::Error => e
        Rails.logger.error("[OpenAI adapter] converse HTTP error: #{e.message}")
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

      def reasoning_effort(thinking)
        # Map a token budget onto OpenAI's coarse effort levels.
        return "high" if thinking.to_i >= 8000
        return "low" if thinking.to_i.positive? && thinking.to_i < 2000
        "medium"
      end

      def parse_converse(body)
        message = body.dig("choices", 0, "message") || {}
        tool_calls = Array(message["tool_calls"]).map do |tc|
          fn = tc["function"] || {}
          args = JSON.parse(fn["arguments"].presence || "{}") rescue {}
          Ai::ChatResult::ToolCall.new(id: tc["id"], name: fn["name"], arguments: args)
        end
        Ai::ChatResult.new(
          text: message["content"],
          tool_calls: tool_calls,
          thinking: message["reasoning_content"],
          stop_reason: body.dig("choices", 0, "finish_reason"),
          usage: { input: body.dig("usage", "prompt_tokens"), output: body.dig("usage", "completion_tokens") }
        )
      end

      def translate_messages(messages)
        messages.flat_map { |msg| translate_message(msg) }
      end

      # Returns either one message hash or (for a tool-result batch) an array.
      def translate_message(msg)
        if msg[:results]
          msg[:results].map { |r| { role: "tool", tool_call_id: r[:tool_call_id], content: r[:content].to_s } }
        elsif msg[:tool_calls]&.any?
          calls = msg[:tool_calls].map do |tc|
            { id: tc["id"], type: "function",
              function: { name: tc["name"], arguments: (tc["arguments"] || {}).to_json } }
          end
          { role: "assistant", content: msg[:content], tool_calls: calls }
        elsif msg[:parts]
          { role: msg[:role] || "user", content: msg[:parts].map { |part| translate_part(part) } }
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
