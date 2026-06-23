class CreateSkimDecisions < ActiveRecord::Migration[8.1]
  def change
    create_table :skim_decisions do |t|
      t.references :user, null: false, foreign_key: true
      t.references :workspace, null: false, foreign_key: true
      t.references :contact, foreign_key: true
      t.string :sender_domain
      t.string :category
      t.string :action, null: false
      t.bigint :email_message_id

      t.timestamps
    end

    # The learning query pulls a user's recent decisions in one shot, then tallies
    # by sender / domain / category in memory — so it reads by (user, recency).
    add_index :skim_decisions, [ :user_id, :created_at ]
  end
end
