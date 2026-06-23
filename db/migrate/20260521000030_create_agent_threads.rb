class CreateAgentThreads < ActiveRecord::Migration[8.1]
  def change
    create_table :agent_threads do |t|
      t.references :user, null: false, foreign_key: true
      t.string :title, null: false
      t.timestamps
    end

    add_index :agent_threads, [ :user_id, :created_at ]
  end
end
