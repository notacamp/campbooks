# frozen_string_literal: true

# Structured turn data for Scout's agent loop:
#   ai_thinking — the model's reasoning trace (shown collapsed in the UI).
#   steps       — ordered [{tool_call:, args:, result:}] taken to produce the
#                 reply, so history replays as native tool_use/tool_result blocks
#                 and the UI can show "Searched email → 12 results".
class AddThinkingAndStepsToAgentMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :agent_messages, :ai_thinking, :text
    add_column :agent_messages, :steps, :jsonb, default: [], null: false
  end
end
