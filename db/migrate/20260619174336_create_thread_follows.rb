class CreateThreadFollows < ActiveRecord::Migration[8.1]
  def change
    create_table :thread_follows do |t|
      # No standalone user_id index — the composite unique below covers it.
      t.references :user, null: false, foreign_key: true, index: false
      t.references :agent_thread, null: false, foreign_key: true

      t.timestamps
    end

    add_index :thread_follows, [ :user_id, :agent_thread_id ], unique: true
  end
end
