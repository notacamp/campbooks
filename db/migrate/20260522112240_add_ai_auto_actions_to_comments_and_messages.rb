class AddAiAutoActionsToCommentsAndMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :email_comments, :ai_auto_actions, :jsonb, default: [], null: false
    add_column :agent_messages, :ai_auto_actions, :jsonb, default: [], null: false
  end
end
