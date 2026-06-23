class CreateContacts < ActiveRecord::Migration[8.0]
  def change
    create_table :contacts do |t|
      t.string :email, null: false
      t.string :name
      t.string :organization
      t.string :relationship_type
      t.text :context_summary
      t.jsonb :communication_patterns, default: {}
      t.text :raw_analysis
      t.datetime :analyzed_at
      t.datetime :last_email_at
      t.integer :email_count, default: 0
      t.references :email_account, foreign_key: true, null: true

      t.timestamps
    end

    add_index :contacts, :email, unique: true
    add_index :contacts, :relationship_type
    add_index :contacts, :last_email_at
    add_index :contacts, [ :email_account_id, :email ], unique: true
  end
end
