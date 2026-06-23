class CreateSearchRecords < ActiveRecord::Migration[8.1]
  def change
    create_table :search_records do |t|
      t.references :organization, null: false, foreign_key: true
      t.string :searchable_type, null: false
      t.bigint :searchable_id, null: false
      t.text :title
      t.text :content_preview
      t.text :tags, array: true, default: []
      t.jsonb :filter_data, null: false, default: {}
      t.column :title_embedding, :vector, limit: 1536
      t.column :content_embedding, :vector, limit: 1536
      t.string :embedding_model
      t.datetime :source_created_at
      t.datetime :source_updated_at
      t.datetime :indexed_at
      t.timestamps
    end

    add_index :search_records, [ :searchable_type, :searchable_id ], unique: true
    add_index :search_records, :source_created_at
    add_index :search_records, :filter_data, using: :gin
    add_index :search_records, :tags, using: :gin
  end
end
