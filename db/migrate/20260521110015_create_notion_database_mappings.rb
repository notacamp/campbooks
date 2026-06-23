class CreateNotionDatabaseMappings < ActiveRecord::Migration[8.1]
  def change
    create_table :notion_database_mappings do |t|
      t.references :document_type, null: false, foreign_key: true, index: { unique: true }
      t.string :notion_database_id, null: false
      t.string :notion_database_name
      t.jsonb :field_mappings, default: {}
      t.boolean :push_enabled, default: false, null: false
      t.boolean :pull_enabled, default: false, null: false

      t.timestamps
    end
  end
end
