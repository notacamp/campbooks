class AddReadToAgentMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :agent_messages, :read, :boolean, default: false, null: false

    reversible do |dir|
      dir.up do
        execute <<~SQL.squish
          UPDATE agent_messages SET read = true WHERE author_type = 1
        SQL
      end
    end

    add_index :agent_messages, %i[agent_thread_id author_type],
              where: "read = false",
              name: "idx_agent_messages_unread"
  end
end
