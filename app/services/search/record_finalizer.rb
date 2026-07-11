# frozen_string_literal: true

module Search
  # Core finalization logic for a Searchable: compute the weighted-average
  # content embedding from its fresh chunks, optionally embed its title, and
  # upsert the SearchRecord.
  #
  # Extracted from FinalizeSearchRecordJob so WorkspaceReembedJob can call it
  # with a precomputed title vector (batched in a single embed_batch call per
  # phase-2 iteration) rather than issuing one embed call per record.
  class RecordFinalizer
    # +searchable+   - any Searchable record (EmailMessage, Contact, …)
    # +entry+        - Ai::EmbeddingModels entry; defaults to the workspace's.
    # +title_vector+ - :compute  → embed the title here (default, preserves
    #                              FinalizeSearchRecordJob's original behaviour);
    #                  a vector   → use as-is (precomputed by the caller);
    #                  nil        → no title embedding.
    def self.call(searchable, entry: nil, title_vector: :compute)
      new(searchable, entry: entry, title_vector: title_vector).call
    end

    def initialize(searchable, entry: nil, title_vector: :compute)
      @searchable   = searchable
      @workspace    = searchable.searchable_workspace
      @entry        = entry || @workspace.embedding_model_entry
      @title_vector = title_vector
    end

    def call
      searchable_type = @searchable.class.name
      searchable_id   = @searchable.id

      # Only average over chunks that are already fresh for this entry — stale
      # chunks carry vectors in a different embedding space.
      chunks = SearchChunk.for_searchable(searchable_type, searchable_id)
                          .fresh_for(@entry)
                          .order(:position)

      return if chunks.empty?

      content_vec = compute_weighted_average(chunks)

      title       = @searchable.searchable_title
      title_vec   = resolve_title_vector(title)

      record = SearchRecord.find_or_initialize_by(
        searchable_type: searchable_type,
        searchable_id:   searchable_id
      )

      record.workspace         = @workspace
      record.title             = title
      record.content_preview   = @searchable.searchable_content_preview
      record.tags              = @searchable.searchable_tags
      record.filter_data       = @searchable.searchable_filter_data
      record.source_created_at = @searchable.searchable_source_created_at
      record.source_updated_at = @searchable.searchable_source_updated_at
      record.indexed_at        = Time.current

      # assign_embedding writes the entry-appropriate dim column, nils the
      # others, and stamps embedding_model — both calls use the same entry so
      # the final stamp is consistent regardless of which call runs last.
      record.assign_embedding(:content_embedding, content_vec, entry: @entry)
      record.assign_embedding(:title_embedding,   title_vec,   entry: @entry)

      record.save!
    end

    private

    # Token-count-weighted average of the per-chunk vectors for this entry's
    # dimensions. Returns nil when all chunk vectors are absent (shouldn't
    # happen for a fresh_for batch, but guard defensively).
    def compute_weighted_average(chunks)
      total_weight = chunks.sum { |c| (c.token_count || 1).to_f }
      return nil if total_weight.zero?

      dimension    = @entry.dimensions
      weighted_sum = Array.new(dimension, 0.0)

      chunks.each do |chunk|
        weight = (chunk.token_count || 1).to_f / total_weight
        vec    = chunk.embedding_vector(:embedding, dimension)
        next unless vec

        vec.each_with_index { |val, i| weighted_sum[i] += val * weight }
      end

      weighted_sum
    end

    def resolve_title_vector(title)
      return @title_vector unless @title_vector == :compute

      # :compute — embed the title inline (FinalizeSearchRecordJob behaviour).
      EmbeddingService.embed(title, workspace: @workspace, entry: @entry) if title.present?
    end
  end
end
