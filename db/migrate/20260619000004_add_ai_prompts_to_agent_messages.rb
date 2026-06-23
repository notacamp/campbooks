class AddAiPromptsToAgentMessages < ActiveRecord::Migration[8.1]
  def change
    # Follow-up prompt suggestions Scout offers after a reply (array of short
    # strings the user can tap to continue the conversation). Mirrors the
    # existing ai_suggested_actions / ai_auto_actions jsonb columns.
    add_column :agent_messages, :ai_prompts, :jsonb, default: [], null: false
  end
end
