class AllowNullEmbeddingOnSearchChunks < ActiveRecord::Migration[8.1]
  # Chunks are created first, then embedded asynchronously by EmbedChunkJob.
  # The NOT NULL constraint made the two-phase flow impossible and caused every
  # EmbedSearchableJob to fail with a NotNullViolation.
  def change
    change_column_null :search_chunks, :embedding, true
  end
end
