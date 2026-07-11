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

  # Returns the text to embed for +tag+: "name -- prompt" when the tag has a
  # prompt, otherwise just the name. This is also the source for content_hash.
  # Tag#prompt already returns plain text (it's overridden on the model), so
  # use it directly — calling .to_plain_text on the String raises.
  def self.embedding_text_for(tag)
    parts = [ tag.name ]
    parts << tag.prompt if tag.respond_to?(:prompt) && tag.prompt.present?
    parts.join(" -- ")
  end
end
