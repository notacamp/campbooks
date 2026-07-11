# frozen_string_literal: true

# Mixin for models that store embeddings in multiple dimension-specific columns.
#
# Usage:
#   dimensioned_embeddings(
#     embedding:         { 1536 => :embedding,         1024 => :embedding_1024,         3072 => :embedding_3072 },
#     content_embedding: { 1536 => :content_embedding, 1024 => :content_embedding_1024, 3072 => :content_embedding_3072 },
#     title_embedding:   { 1536 => :title_embedding,   1024 => :title_embedding_1024,   3072 => :title_embedding_3072 }
#   )
#
# The first declared kind is treated as the default for scope helpers (fresh_for / stale_for)
# when no kind: argument is supplied.
module DimensionedEmbeddings
  extend ActiveSupport::Concern

  included do
    # Populated by .dimensioned_embeddings; keyed by kind Symbol.
    class_attribute :_dimensioned_embedding_kinds, default: {}
    class_attribute :_default_embedding_kind, default: nil
  end

  class_methods do
    # Declare the embedding kinds and their dimension->column maps.
    # +kinds+ is a Hash of { kind_symbol => { dims_int => column_symbol, ... } }.
    def dimensioned_embeddings(kinds)
      self._dimensioned_embedding_kinds = kinds.freeze
      self._default_embedding_kind = kinds.keys.first
    end

    # Returns the column Symbol for +kind+ at +dimensions+.
    # Raises ArgumentError for unknown kind or dimensions.
    def embedding_column_for(kind, dimensions)
      dim_map = _dimensioned_embedding_kinds[kind]
      raise ArgumentError, "Unknown embedding kind #{kind.inspect} for #{name}" unless dim_map

      col = dim_map[dimensions]
      raise ArgumentError, "Unknown dimensions #{dimensions} for kind #{kind.inspect} on #{name}" unless col

      col
    end

    # Scope: rows whose stamp matches +entry+ AND whose entry-dims column is NOT NULL.
    #
    # "Matches" for the DEFAULT entry includes legacy rows (embedding_model IS NULL)
    # because those were written before per-model stamping existed and carry the
    # default model's vectors.
    def fresh_for(entry, kind: _default_embedding_kind)
      col = embedding_column_for(kind, entry.dimensions)

      if entry.default?
        # Legacy rows (NULL stamp) are considered fresh for the default entry.
        where("(embedding_model = :m OR embedding_model IS NULL) AND #{col} IS NOT NULL", m: entry.model)
      else
        where("embedding_model = :m AND #{col} IS NOT NULL", m: entry.model)
      end
    end

    # Scope: the exact complement of fresh_for — rows that need (re)embedding.
    def stale_for(entry, kind: _default_embedding_kind)
      col = embedding_column_for(kind, entry.dimensions)

      if entry.default?
        # Stale when stamp is set to something OTHER than the model, OR the column is null.
        where("(embedding_model IS NOT NULL AND embedding_model <> :m) OR #{col} IS NULL", m: entry.model)
      else
        # Stale when stamp is null (unembedded), points to a different model, or the column is null.
        where("embedding_model IS NULL OR embedding_model <> :m OR #{col} IS NULL", m: entry.model)
      end
    end
  end

  # Assign +vector+ to the entry-appropriate column for +kind+, nil out all OTHER
  # dimension columns for that kind, and stamp embedding_model with entry.model.
  # Pure assignment — does not call save.
  def assign_embedding(kind, vector, entry:)
    kind_map = self.class._dimensioned_embedding_kinds[kind]
    raise ArgumentError, "Unknown embedding kind #{kind.inspect} for #{self.class.name}" unless kind_map

    kind_map.each do |dims, col|
      send(:"#{col}=", dims == entry.dimensions ? vector : nil)
    end

    self.embedding_model = entry.model
  end

  # Returns the vector stored in the column for +kind+ at +dimensions+.
  def embedding_vector(kind, dimensions)
    col = self.class.embedding_column_for(kind, dimensions)
    send(col)
  end

  # True when this row's stamp is consistent with +entry+.
  # Legacy rows (embedding_model IS NULL) are considered matching for the default entry.
  def stamp_matches?(entry)
    embedding_model == entry.model || (embedding_model.nil? && entry.default?)
  end
end
