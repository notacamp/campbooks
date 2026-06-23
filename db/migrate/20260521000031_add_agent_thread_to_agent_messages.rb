class AddAgentThreadToAgentMessages < ActiveRecord::Migration[8.1]
  def change
    add_reference :agent_messages, :agent_thread, foreign_key: true
    add_index :agent_messages, [ :agent_thread_id, :created_at ]
  end
end
