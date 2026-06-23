class CreateSearchChunks < ActiveRecord::Migration[8.1]
  def change
    create_table :search_chunks do |t|
      t.references :organization, null: false, foreign_key: true
      t.string :searchable_type, null: false
      t.bigint :searchable_id, null: false
      t.text :content, null: false
      t.string :chunk_type, null: false, default: "text"
      t.integer :position, null: false, default: 0
      t.integer :token_count
      t.jsonb :metadata, null: false, default: {}
      t.column :embedding, :vector, limit: 1536, null: false
      t.string :embedding_model
      t.timestamps
    end

    add_index :search_chunks, [ :searchable_type, :searchable_id ]
    add_index :search_chunks, :metadata, using: :gin
  end
end
