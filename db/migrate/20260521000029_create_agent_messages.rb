class CreateAgentMessages < ActiveRecord::Migration[8.1]
  def change
    create_table :agent_messages do |t|
      t.references :user, null: false, foreign_key: true
      t.text :content, null: false
      t.integer :author_type, null: false, default: 0
      t.jsonb :ai_suggested_actions, null: false, default: []
      t.timestamps
    end

    add_index :agent_messages, [ :user_id, :created_at ]
  end
end
