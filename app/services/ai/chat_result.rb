# frozen_string_literal: true

module Ai
  # Provider-neutral result of one assistant turn from an adapter's `converse`.
  # Replaces the old "return a raw String" contract so the agent loop can reason
  # over native tool calls and thinking without parsing JSON out of prose.
  #
  #   result.tool_calls? # => the model wants to run tools; execute and continue
  #   result.text        # => the assistant's natural-language content (may be nil)
  #   result.thinking    # => reasoning trace, when the model/provider exposes it
  #
  # A ToolCall carries the provider-assigned `id` (echoed back in the tool result
  # so the model can match call→result) plus the parsed `name`/`arguments`.
  class ChatResult
    ToolCall = Data.define(:id, :name, :arguments) do
      def to_h = { "id" => id, "name" => name, "arguments" => arguments }
    end

    attr_reader :text, :tool_calls, :thinking, :stop_reason, :usage

    def initialize(text: nil, tool_calls: [], thinking: nil, stop_reason: nil, usage: {})
      @text = text.presence
      @tool_calls = Array(tool_calls)
      @thinking = thinking.presence
      @stop_reason = stop_reason
      @usage = usage || {}
    end

    def tool_calls? = @tool_calls.any?

    # Rebuild a neutral assistant message for replaying this turn back to the
    # model on the next loop iteration (so the model sees its own tool calls).
    def to_assistant_message
      { role: "assistant", content: @text, thinking: @thinking,
        tool_calls: @tool_calls.map(&:to_h) }
    end
  end
end
