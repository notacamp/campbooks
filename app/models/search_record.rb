class SearchRecord < ApplicationRecord
  belongs_to :workspace
  belongs_to :searchable, polymorphic: true

  has_neighbors :title_embedding
  has_neighbors :content_embedding

  scope :by_type, ->(type) { where(searchable_type: type) }
  scope :with_tags, ->(tag_names) { where("tags && ARRAY[?]::text[]", Array(tag_names)) }

  def searchable_title_match?(query_embedding)
    return nil unless title_embedding.present?
    cosine_similarity(title_embedding, query_embedding)
  end

  private

  def cosine_similarity(vec_a, vec_b)
    dot_product = vec_a.zip(vec_b).sum { |a, b| a * b }
    magnitude_a = Math.sqrt(vec_a.sum { |a| a * a })
    magnitude_b = Math.sqrt(vec_b.sum { |b| b * b })
    return 0.0 if magnitude_a.zero? || magnitude_b.zero?
    dot_product / (magnitude_a * magnitude_b)
  end
end
