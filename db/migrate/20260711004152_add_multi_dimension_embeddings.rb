# frozen_string_literal: true

class AddMultiDimensionEmbeddings < ActiveRecord::Migration[8.1]
  def change
    # Per-workspace embedding model selection (nil = use DEFAULT).
    add_column :workspaces, :embedding_model, :string

    # Additional dimension columns for search_chunks.
    add_column :search_chunks, :embedding_1024, :vector, limit: 1024
    add_column :search_chunks, :embedding_3072, :vector, limit: 3072

    # Additional dimension columns for search_records.
    add_column :search_records, :content_embedding_1024, :vector, limit: 1024
    add_column :search_records, :content_embedding_3072, :vector, limit: 3072
    add_column :search_records, :title_embedding_1024, :vector, limit: 1024
    add_column :search_records, :title_embedding_3072, :vector, limit: 3072

    # Additional dimension columns for search_tag_embeddings.
    add_column :search_tag_embeddings, :embedding_1024, :vector, limit: 1024
    add_column :search_tag_embeddings, :embedding_3072, :vector, limit: 3072

    # Allow NULL on the legacy 1536 column — new workspaces on a non-1536 model
    # will never populate it, and the NOT NULL constraint would block inserts.
    change_column_null :search_tag_embeddings, :embedding, true

    # HNSW cosine indexes on the 1024 columns only.
    # NOTE: pgvector's HNSW index implementation caps supported dimensions at
    # 2000 for vector_cosine_ops (and other ops), so 3072-dim columns cannot
    # have HNSW indexes — a 3072-dim HNSW build would raise at creation time.
    # Exact KNN scans on 3072 columns are still possible but slower (no index).
    add_index :search_chunks, :embedding_1024,
              name: "idx_search_chunks_embedding_1024_hnsw",
              using: :hnsw, opclass: :vector_cosine_ops

    add_index :search_records, :content_embedding_1024,
              name: "idx_search_records_content_1024_hnsw",
              using: :hnsw, opclass: :vector_cosine_ops

    add_index :search_records, :title_embedding_1024,
              name: "idx_search_records_title_1024_hnsw",
              using: :hnsw, opclass: :vector_cosine_ops

    add_index :search_tag_embeddings, :embedding_1024,
              name: "idx_search_tag_embeddings_1024_hnsw",
              using: :hnsw, opclass: :vector_cosine_ops
  end
end
