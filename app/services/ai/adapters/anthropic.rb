module Ai
  module Adapters
    class Anthropic < Base
      # Anthropic's stable API version. PDF document blocks (type: "document")
      # are GA under this version — no beta header needed. The previous value
      # ("2024-02-15") is not a valid anthropic-version and made every call
      # (text, image, and document) 400, which silently broke document analysis
      # and ContactAnalyzer whenever they were routed to Claude.
      ANTHROPIC_VERSION = "2023-06-01"

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
          req.options.context = { model: model.to_s }
        end

        extract_text(JSON.parse(response.body))
      rescue Faraday::Error => e
        Rails.logger.error("[Anthropic adapter] HTTP error: #{e.message}")
        raise
      end

      def supports_tools? = true

      # Extended thinking is available on Claude 3.7 and the 4.x family.
      def supports_thinking?(model)
        model.to_s.match?(/claude-(3-7|sonnet-4|opus-4|haiku-4)|claude-4/)
      end

      def converse(system:, messages:, model:, max_tokens:, temperature: 0.0, tools: [], thinking: nil)
        body = { model: model, system: system, messages: translate_messages(messages) }

        # Anthropic requires prior `thinking` blocks (with signatures) to be
        # replayed alongside tool_use turns. To keep the turn model simple we
        # only think on tool-free turns (the final answer); reasoning-model
        # providers handle think-during-tools on their own path.
        think = thinking && supports_thinking?(model) && tools.empty?
        if think
          budget = [ thinking.to_i, 1024 ].max
          body[:max_tokens] = max_tokens + budget          # output room beyond the thinking budget
          body[:thinking] = { type: "enabled", budget_tokens: budget }
          # temperature must be left at its default (1) when thinking is enabled
        else
          body[:max_tokens] = max_tokens
          body[:temperature] = temperature > 0 ? temperature : 0.001
        end
        body[:tools] = tools.map { |t| { name: t[:name], description: t[:description], input_schema: t[:parameters] } } if tools.any?

        parse_converse(post_json(@endpoint_url, body))
      rescue Faraday::Error => e
        Rails.logger.error("[Anthropic adapter] converse HTTP error: #{e.message}")
        raise
      end

      private

      def parse_converse(body)
        blocks = Array(body["content"])
        text = blocks.select { |b| b["type"] == "text" }.map { |b| b["text"] }.join("\n").presence
        thinking = blocks.select { |b| b["type"] == "thinking" }.map { |b| b["thinking"] }.join("\n").presence
        tool_calls = blocks.select { |b| b["type"] == "tool_use" }.map do |b|
          Ai::ChatResult::ToolCall.new(id: b["id"], name: b["name"], arguments: b["input"] || {})
        end
        Ai::ChatResult.new(
          text: text, tool_calls: tool_calls, thinking: thinking,
          stop_reason: body["stop_reason"],
          usage: { input: body.dig("usage", "input_tokens"), output: body.dig("usage", "output_tokens") }
        )
      end

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
        if msg[:results]
          # A batch of tool results becomes one user turn of tool_result blocks.
          { role: "user", content: msg[:results].map { |r|
            { type: "tool_result", tool_use_id: r[:tool_call_id], content: r[:content].to_s }
          } }
        elsif msg[:tool_calls]&.any?
          blocks = []
          blocks << { type: "text", text: msg[:content] } if msg[:content].present?
          msg[:tool_calls].each { |tc| blocks << { type: "tool_use", id: tc["id"], name: tc["name"], input: tc["arguments"] || {} } }
          { role: "assistant", content: blocks }
        elsif msg[:parts]
          # Multi-part message (vision/docs)
          { role: msg[:role] || "user", content: msg[:parts].map { |part| translate_part(part) } }
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
