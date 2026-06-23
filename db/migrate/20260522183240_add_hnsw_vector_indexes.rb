class AddHnswVectorIndexes < ActiveRecord::Migration[8.1]
  def up
    execute <<-SQL
      CREATE INDEX idx_search_chunks_embedding_hnsw
        ON search_chunks USING hnsw (embedding vector_cosine_ops)
        WITH (m = 16, ef_construction = 64);
    SQL

    execute <<-SQL
      CREATE INDEX idx_search_records_title_hnsw
        ON search_records USING hnsw (title_embedding vector_cosine_ops)
        WITH (m = 16, ef_construction = 64);
    SQL

    execute <<-SQL
      CREATE INDEX idx_search_records_content_hnsw
        ON search_records USING hnsw (content_embedding vector_cosine_ops)
        WITH (m = 16, ef_construction = 64);
    SQL

    execute <<-SQL
      CREATE INDEX idx_search_tag_embeddings_hnsw
        ON search_tag_embeddings USING hnsw (embedding vector_cosine_ops)
        WITH (m = 16, ef_construction = 64);
    SQL
  end

  def down
    remove_index :search_chunks, name: :idx_search_chunks_embedding_hnsw if index_exists?(:search_chunks, :embedding, name: :idx_search_chunks_embedding_hnsw)
    remove_index :search_records, name: :idx_search_records_title_hnsw if index_exists?(:search_records, :title_embedding, name: :idx_search_records_title_hnsw)
    remove_index :search_records, name: :idx_search_records_content_hnsw if index_exists?(:search_records, :content_embedding, name: :idx_search_records_content_hnsw)
    remove_index :search_tag_embeddings, name: :idx_search_tag_embeddings_hnsw if index_exists?(:search_tag_embeddings, :embedding, name: :idx_search_tag_embeddings_hnsw)
  end
end
