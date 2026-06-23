class SearchChunk < ApplicationRecord
  belongs_to :workspace
  belongs_to :searchable, polymorphic: true

  has_neighbors :embedding

  scope :for_searchable, ->(type, id) { where(searchable_type: type, searchable_id: id) }
end
