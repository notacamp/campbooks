class AddSystemPromptToAiConfigurations < ActiveRecord::Migration[8.1]
  def change
    add_column :ai_configurations, :system_prompt, :text
  end
end
