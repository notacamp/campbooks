# frozen_string_literal: true

module Scout
  # The Global Scout agent loop, built on native tool calling.
  #
  # Replaces the old JSON-in-prose protocol: the model emits real tool_use turns,
  # we execute read tools and feed structured tool_result turns back, and we stop
  # when the model returns a final text answer (model-driven, not a hard 3-cap
  # that errored out). Destructive tools are never executed from model output —
  # they're surfaced as one-click confirmations.
  #
  # Emits events via `on_event` (status/tool/proposal) so the UI can show live
  # progress. Returns a Result with the reply, reasoning trace, ordered steps,
  # proposed actions and follow-up prompts.
  class Agent
    PURPOSE = "global_chat"
    MAX_STEPS = 8                 # hard ceiling on tool round-trips; then force an answer
    THINKING_BUDGET = 2048        # reasoning tokens, when the model supports it
    HISTORY = 30                  # max prior turns considered before the char budget trims
    HISTORY_CHAR_BUDGET = 24_000  # ~6k tokens of recent history (older turns dropped)

    Result = Data.define(:reply, :thinking, :steps, :suggested_actions, :prompts, :provenance, :usage)

    def initialize(thread, on_event: nil)
      @thread = thread
      @user = thread.user
      @on_event = on_event
      @steps = []
      @proposals = []
      @thinking = nil
    end

    def run(user_text)
      config = Ai::Configuration.for(PURPOSE)
      return nil unless config
      return legacy_fallback(user_text) unless config[:adapter].supports_tools?

      messages = initial_messages(user_text)
      final = drive(config, messages)
      return nil unless final

      Result.new(
        reply: final.text.to_s.strip,
        thinking: @thinking,
        steps: @steps,
        suggested_actions: @proposals,
        prompts: best_effort_prompts(config, final.text),
        provenance: Ai::Provenance.for_purpose(PURPOSE),
        usage: final.usage
      )
    rescue => e
      Rails.logger.error("[Scout::Agent] #{e.class}: #{e.message}\n#{e.backtrace&.first(8)&.join("\n")}")
      nil
    end

    private

    # Run the converse → execute-tools → converse loop until a text answer.
    def drive(config, messages)
      tools = ToolRegistry.provider_payload

      MAX_STEPS.times do |i|
        last_turn = i == MAX_STEPS - 1
        result = converse(config, messages, tools: last_turn ? [] : tools)
        return nil unless result

        @thinking = [ @thinking, result.thinking ].compact.join("\n\n").presence
        return result unless result.tool_calls?
        return result if last_turn # cap hit but model still wanted tools — answer with what it has

        messages << result.to_assistant_message
        messages << { role: "tool", results: result.tool_calls.map { |tc| handle_tool_call(tc) } }
      end
      nil
    end

    def converse(config, messages, tools:)
      config[:adapter].converse(
        system: system_prompt, messages: messages, model: config[:model],
        max_tokens: config[:max_tokens], temperature: config[:temperature],
        tools: tools, thinking: THINKING_BUDGET
      )
    rescue Faraday::Error => e
      Rails.logger.error("[Scout::Agent] adapter error: #{e.message}")
      nil
    end

    # Read tools run now; confirm tools are recorded as proposals and the model
    # is told they await the user — it must not claim they're done.
    def handle_tool_call(call)
      tool = ToolRegistry.find(call.name)
      unless tool
        return { tool_call_id: call.id, content: { error: "Unknown tool: #{call.name}" }.to_json }
      end

      if tool.confirm?
        @proposals << { "tool" => call.name, "args" => call.arguments, "label" => proposal_label(call) }
        emit(:proposal, call.name)
        body = { status: "proposed_to_user", note: "Awaiting the user's one-click confirmation." }
      else
        emit(:tool, call.name)
        body = ToolRegistry.run(call.name, call.arguments)
        @steps << { "tool" => call.name, "args" => call.arguments, "result" => summarize(body) }
      end
      { tool_call_id: call.id, content: body.to_json }
    end

    def initial_messages(user_text)
      rows = @thread.agent_messages.chronological.last(HISTORY).map do |m|
        { role: m.from_user? ? "user" : "assistant", content: m.content.to_s }
      end
      rows << { role: "user", content: user_text } unless rows.last&.dig(:content) == user_text
      budget_history(rows)
    end

    # Keep the most recent turns within a character budget instead of a blind
    # last(N): a long back-and-forth can't silently overflow the context window.
    # (A first step toward summarisation — see the PR notes.)
    def budget_history(rows)
      kept = []
      total = 0
      rows.reverse_each do |row|
        size = row[:content].length
        break if kept.size >= 2 && total + size > HISTORY_CHAR_BUDGET

        kept.unshift(row)
        total += size
      end
      kept
    end

    def system_prompt
      stats = Tools::SystemStats.call rescue { error: "snapshot unavailable" }
      <<~PROMPT
        #{Ai::ChatService.base_prompt(PURPOSE)}

        You are Scout in your home base — a sharp, proactive operator with global
        access to the user's email, documents and contacts. Lead with the answer,
        be concise and opinionated, never invent data.

        You have native tools. Call read tools (query_*, generate_report) freely to
        get exact, current numbers before stating them — never guess. Destructive
        tools (bulk_archive, bulk_tag, reclassify) only PROPOSE an action for the
        user to confirm with one click: never say you have already done them.

        Live snapshot (orientation only; run tools for exact figures):
        #{stats.to_json}

        Reply in clean markdown. No filler openers, no emoji headers. End on substance.
        #{Ai::Configuration.user_prompt_suffix(PURPOSE)}
      PROMPT
    end

    # One cheap, best-effort call for the follow-up prompt chips. Never blocks the
    # reply: any failure just yields no chips.
    def best_effort_prompts(config, reply)
      return [] if reply.blank?

      raw = config[:adapter].chat(
        system: "Return ONLY a JSON array of 2-4 short next-step prompts phrased as the user would tap them. No prose.",
        messages: [ { role: "user", content: "Answer was:\n#{reply.to_s.truncate(1500)}\n\nJSON array:" } ],
        model: config[:model], max_tokens: 200, temperature: 0.3
      )
      Array(JSON.parse(raw[/\[.*\]/m].to_s)).map(&:to_s).reject(&:blank?).first(4)
    rescue => e
      Rails.logger.warn("[Scout::Agent] follow-up prompts skipped: #{e.message}")
      []
    end

    def proposal_label(call)
      ToolRegistry.find(call.name)&.description.to_s.split(".").first.presence || call.name.tr("_", " ")
    end

    # Keep tool results small when persisted as a step (the full payload already
    # went to the model); store counts/keys, not whole record dumps.
    def summarize(body)
      return body unless body.is_a?(Hash)
      body.transform_values { |v| v.is_a?(Array) ? "#{v.size} items" : v }
    end

    def emit(type, label)
      @on_event&.call(type, label)
    rescue => e
      Rails.logger.warn("[Scout::Agent] event callback failed: #{e.message}")
    end

    # Workspaces whose model can't do native tool calling keep the previous
    # JSON-protocol service unchanged.
    def legacy_fallback(user_text)
      svc = Ai::GlobalChatService.new(@thread)
      out = svc.reply_to(@thread.agent_messages.chronological.last, on_status: ->(l) { emit(:status, l) })
      return nil unless out

      Result.new(
        reply: out[:reply], thinking: nil, steps: [],
        suggested_actions: out[:suggested_actions] || [], prompts: out[:prompts] || [],
        provenance: out[:provenance], usage: {}
      )
    end
  end
end
