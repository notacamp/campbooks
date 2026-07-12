# frozen_string_literal: true

class SearchChunk < ApplicationRecord
  include DimensionedEmbeddings

  belongs_to :workspace
  belongs_to :searchable, polymorphic: true

  has_neighbors :embedding
  has_neighbors :embedding_1024
  has_neighbors :embedding_3072

  dimensioned_embeddings(
    embedding: { 1536 => :embedding, 1024 => :embedding_1024, 3072 => :embedding_3072 }
  )

  scope :for_searchable, ->(type, id) { where(searchable_type: type, searchable_id: id) }
end
