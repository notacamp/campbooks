class AddAiSuggestedActions < ActiveRecord::Migration[8.1]
  def change
    add_column :email_messages, :ai_suggested_actions, :jsonb, default: [], null: false
    add_column :email_comments, :ai_suggested_actions, :jsonb, default: [], null: false
  end
end
