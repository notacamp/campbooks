class SearchTagEmbedding < ApplicationRecord
  belongs_to :workspace
  belongs_to :tag

  has_neighbors :embedding
end
