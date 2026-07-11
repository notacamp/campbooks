# frozen_string_literal: true

class SearchTagEmbedding < ApplicationRecord
  include DimensionedEmbeddings

  belongs_to :workspace
  belongs_to :tag

  has_neighbors :embedding
  has_neighbors :embedding_1024
  has_neighbors :embedding_3072

  dimensioned_embeddings(
    embedding: { 1536 => :embedding, 1024 => :embedding_1024, 3072 => :embedding_3072 }
  )
end
