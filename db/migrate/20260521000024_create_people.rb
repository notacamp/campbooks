class CreatePeople < ActiveRecord::Migration[8.1]
  def change
    create_table :people do |t|
      t.string :name
      t.string :organization
      t.string :relationship_type
      t.text :context_summary
      t.jsonb :communication_patterns, default: {}
      t.text :raw_analysis
      t.datetime :analyzed_at
      t.timestamps
    end

    add_index :people, :name
    add_index :people, :organization
    add_index :people, :relationship_type

    add_reference :contacts, :person, foreign_key: true, null: true
    add_reference :contacts, :suggested_person, foreign_key: { to_table: :people }, null: true
    add_column :contacts, :suggested_reason, :text
    add_column :contacts, :suggested_confidence, :float
  end
end
