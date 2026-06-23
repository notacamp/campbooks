class CreateSearchTagEmbeddings < ActiveRecord::Migration[8.1]
  def change
    create_table :search_tag_embeddings do |t|
      t.references :organization, null: false, foreign_key: true
      t.references :tag, null: false, foreign_key: true, index: { unique: true }
      t.column :embedding, :vector, limit: 1536, null: false
      t.string :embedding_model
      t.string :content_hash
      t.timestamps
    end
  end
end
