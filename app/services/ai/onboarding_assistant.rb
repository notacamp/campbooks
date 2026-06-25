module Ai
  class OnboardingAssistant
    def initialize(workspace)
      @workspace = workspace
    end

    # True when an AI provider is configured (a DB AiConfiguration or, in
    # self-hosted mode, an env API key). Lets callers fail fast with a helpful
    # message instead of starting a conversation that can't be answered.
    def available?
      resolve_config.present?
    end

    def suggest_document_types
      config = resolve_config
      return [] unless config

      existing = @workspace.document_types.pluck(:name)
      prompt = document_types_prompt(existing)

      response = config[:adapter].chat(
        system: "You are a business setup assistant. Respond with valid JSON only, no other text.",
        messages: [ { role: :user, content: prompt } ],
        model: config[:model],
        max_tokens: config[:max_tokens],
        temperature: 0.3
      )

      parse_suggestions(response, with_schema: true)
    end

    def suggest_tags
      config = resolve_config
      return [] unless config

      existing = @workspace.tags.pluck(:name)
      prompt = tags_prompt(existing)

      response = config[:adapter].chat(
        system: "You are a business setup assistant. Respond with valid JSON only, no other text.",
        messages: [ { role: :user, content: prompt } ],
        model: config[:model],
        max_tokens: config[:max_tokens],
        temperature: 0.3
      )

      parse_suggestions(response, with_schema: false)
    end

    MAX_QUESTIONS = 4

    # Multi-turn setup: Scout asks a few short questions about the business, then
    # proposes a list of document types / tags. `history` is the prior turns as
    # [{ role: "user"|"assistant", content: "..." }] (symbol keys, no system msg).
    # Returns one of:
    #   { type: :question, question:, hint: }
    #   { type: :proposal, items: [..] }
    #   { type: :error,    reason: }
    def conversational_turn(history:, kind:)
      kind = kind.to_sym
      config = resolve_config
      return { type: :error, reason: :no_ai_config } unless config

      asked = history.count { |m| m[:role].to_s == "assistant" }
      messages = history.map { |m| { role: m[:role].to_s, content: m[:content].to_s } }
      messages << { role: "user", content: "I'm ready to set up my #{kind.to_s.humanize.downcase}." } if messages.empty?

      response = config[:adapter].chat(
        system: conversation_system_prompt(kind: kind, force_proposal: asked >= MAX_QUESTIONS),
        messages: messages,
        model: config[:model],
        max_tokens: config[:max_tokens],
        temperature: 0.4
      )

      parse_turn_response(response, kind: kind)
    rescue => e
      Rails.logger.error("[Ai::OnboardingAssistant] conversational_turn error: #{e.message}")
      { type: :error, reason: :api_error }
    end

    # Persist accepted suggestions as DocumentType / Tag records. Idempotent by
    # name; wrapped in a transaction so a bad item never leaves a partial set.
    def self.persist_proposal(workspace:, kind:, items:)
      kind = kind.to_sym
      created = []
      ApplicationRecord.transaction do
        Array(items).each do |item|
          name = item["name"].to_s.strip.downcase
          next if name.blank?

          record =
            if kind == :document_types
              workspace.document_types.find_or_create_by!(name: name) do |dt|
                dt.color = item["color"].presence || "#6366f1"
                dt.prompt = item["prompt"].presence
                dt.extraction_schema = item["extraction_schema"] if item["extraction_schema"].present?
              end
            else
              workspace.tags.find_or_create_by!(name: name) do |tag|
                tag.color = item["color"].presence || "#6366f1"
                tag.prompt = item["prompt"].presence
                tag.source = :local
              end
            end
          created << record
        end
      end
      created
    end

    private

    # System prompt for the conversational flow. Up to MAX_QUESTIONS one-at-a-time
    # questions, then a structured proposal. When `force_proposal` is set we've hit
    # the cap, so demand the proposal now.
    def conversation_system_prompt(kind:, force_proposal: false)
      context = @workspace.workspace_context.presence || @workspace.name
      existing = (kind == :document_types ? @workspace.document_types : @workspace.tags).pluck(:name)
      thing = kind == :document_types ? "document types" : "email tags"
      proposal_key = kind.to_s
      item_shape =
        if kind == :document_types
          %({"name": "...", "color": "#hex", "prompt": "...", "extraction_schema": {...}})
        else
          %({"name": "...", "color": "#hex", "prompt": "..."})
        end
      schema_line =
        if kind == :document_types
          %(        - extraction_schema: a JSON object mapping field names to { "type": "string"|"number"|"date", "description": "..." } for the data Scout should pull from this document.)
        else
          ""
        end

      <<~PROMPT
        You are Scout, the AI assistant for Campbooks, helping a small business set up their #{thing}.

        What we already know about the business:
        "#{context}"

        They already have these #{thing}: #{existing.join(", ").presence || "(none yet)"}

        Your job: understand the business well enough to propose genuinely useful #{thing}, then propose them.
        Ask SHORT, ONE-AT-A-TIME questions (at most #{MAX_QUESTIONS} total) about the documents/emails they actually handle. If you already have enough to be useful, skip straight to the proposal.
        #{force_proposal ? "You have asked enough questions. You MUST emit the proposal now — do not ask another question." : ""}

        Respond with EXACTLY ONE of these two JSON shapes, and NOTHING else (no prose, no markdown fences):

        1) To ask a question:
        {"question": "your question", "hint": "a short example to guide their answer"}

        2) To propose (final):
        {"proposal": {"#{proposal_key}": [#{item_shape}, ...]}}

        Proposal rules:
        - Suggest 3–8 items that are genuinely relevant; never duplicate the existing ones above.
        - name: snake_case identifier. color: a tasteful hex code. prompt: 1–2 sentences on what it covers / when to apply it.
        #{schema_line}
      PROMPT
    end

    # Parse one turn: a question, a proposal, or an error. Tolerates markdown
    # fences and a bare array (treated as a proposal).
    def parse_turn_response(raw, kind:)
      return { type: :error, reason: :empty } if raw.blank?

      json = raw.strip
      json = json[/```(?:json)?(.*?)```/m, 1]&.strip || json
      data = JSON.parse(json)

      if data.is_a?(Hash) && data["question"].present?
        { type: :question, question: data["question"].to_s, hint: data["hint"].to_s.presence }
      elsif data.is_a?(Hash) && data["proposal"].present?
        items = data.dig("proposal", kind.to_s) || data["proposal"]
        { type: :proposal, items: normalize_items(items, with_schema: kind == :document_types) }
      elsif data.is_a?(Array)
        { type: :proposal, items: normalize_items(data, with_schema: kind == :document_types) }
      else
        { type: :error, reason: :unrecognized }
      end
    rescue JSON::ParserError => e
      Rails.logger.error("[Ai::OnboardingAssistant] turn JSON parse error: #{e.message}")
      { type: :error, reason: :parse_error }
    end

    def resolve_config
      # Any configured text purpose can answer setup questions — don't insist on
      # global_chat specifically, or a workspace that set up AI for, say, email
      # classification would be told "Scout needs an AI provider" here.
      config = Ai::Configuration.for_any(AiConfiguration::TEXT_PURPOSES)
      return config if config

      return nil unless Rails.application.config.self_hosted

      api_key = ENV["OPENAI_API_KEY"] || ENV["DEEPSEEK_API_KEY"] || ENV["ANTHROPIC_API_KEY"]
      return nil unless api_key

      provider = if ENV["OPENAI_API_KEY"]
                   "openai"
      elsif ENV["DEEPSEEK_API_KEY"]
                   "deepseek"
      else
                   "anthropic"
      end

      adapter = Ai::Adapters::Base.for(provider, api_key: api_key)
      {
        adapter: adapter,
        model: AiConfiguration::DEFAULT_MODEL[provider] || "gpt-4o-mini",
        max_tokens: 2000,
        temperature: 0.3
      }
    end

    def document_types_prompt(existing_names)
      context = @workspace.workspace_context.presence || @workspace.name

      <<~PROMPT
        A business with this description is setting up their document management system:

        "#{context}"

        They already have these document types: #{existing_names.join(", ").presence || "(none yet)"}

        Based on the business description, suggest up to 5 additional document types that would be useful.
        Each type should include:
        - name: snake_case identifier (e.g. "payslip", "rental_agreement", "utility_bill")
        - color: a suitable hex color code
        - prompt: a 1-2 sentence description of what this type covers and when to use it
        - extraction_schema: a JSON object mapping field names to { type: "string"|"integer"|"number", description: "..." }

        Only suggest types that are genuinely relevant. If the existing types already cover their needs, return an empty array.

        Respond with a JSON array:
        [{"name": "...", "color": "#hex", "prompt": "...", "extraction_schema": {...}}]
      PROMPT
    end

    def tags_prompt(existing_names)
      context = @workspace.workspace_context.presence || @workspace.name

      <<~PROMPT
        A business with this description is setting up their email tagging system:

        "#{context}"

        They already have these tags: #{existing_names.join(", ").presence || "(none yet)"}

        Based on the business description, suggest up to 5 additional email tags that would help categorize their emails.
        Each tag should include:
        - name: snake_case identifier (e.g. "newsletter", "team_updates", "bills")
        - color: a suitable hex color code
        - prompt: a 1-2 sentence description of when this tag should be applied to an email

        Only suggest tags that are genuinely relevant. If the existing tags already cover their needs, return an empty array.

        Respond with a JSON array:
        [{"name": "...", "color": "#hex", "prompt": "..."}]
      PROMPT
    end

    def parse_suggestions(text, with_schema:)
      return [] if text.blank?

      json = text.strip
      json = json[/```(?:json)?(.*?)```/m, 1]&.strip || json
      normalize_items(JSON.parse(json), with_schema: with_schema)
    rescue JSON::ParserError => e
      Rails.logger.error("[Ai::OnboardingAssistant] JSON parse error: #{e.message}")
      []
    end

    # Coerce a raw array of suggestion hashes into the canonical shape used by the
    # UI and persistence (snake_case name, hex color, prompt, optional schema).
    def normalize_items(data, with_schema:)
      Array(data).filter_map do |item|
        next unless item.is_a?(Hash)
        name = item["name"].to_s.strip.downcase.gsub(/\s+/, "_")
        next if name.blank?

        result = {
          "name" => name,
          "color" => item["color"].presence || generate_color(name),
          "prompt" => item["prompt"].presence || "Suggested: #{name.humanize}"
        }
        result["extraction_schema"] = item["extraction_schema"] if with_schema && item["extraction_schema"].present?
        result
      end
    end

    def generate_color(name)
      hash = name.to_s.bytes.sum
      hue = hash % 360
      "##{hsl_to_hex(hue, 0.65, 0.55)}"
    end

    def hsl_to_hex(h, s, l)
      h = h / 360.0
      c = (1 - (2 * l - 1).abs) * s
      x = c * (1 - ((h * 6) % 2 - 1).abs)
      m = l - c / 2.0
      r, g, b = case (h * 6).floor % 6
      when 0 then [ c, x, 0 ]
      when 1 then [ x, c, 0 ]
      when 2 then [ 0, c, x ]
      when 3 then [ 0, x, c ]
      when 4 then [ x, 0, c ]
      when 5 then [ c, 0, x ]
      end
      [ r, g, b ].map { |v| ((v + m) * 255).round.to_s(16).rjust(2, "0") }.join
    end
  end
end
