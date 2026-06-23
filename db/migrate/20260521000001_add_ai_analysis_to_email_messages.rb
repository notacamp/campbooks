class AddAiAnalysisToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    change_table :email_messages do |t|
      t.text :ai_summary
      t.integer :ai_priority, default: 1, null: false
      t.text :ai_action_prompt
      t.datetime :ai_analyzed_at
      t.boolean :ai_todo_dismissed, default: false, null: false
    end

    add_index :email_messages, :received_at,
      where: "ai_action_prompt IS NOT NULL AND ai_action_prompt != '' AND ai_todo_dismissed = false",
      order: { received_at: :desc },
      name: "idx_email_messages_ai_todos"
  end
end
