class EmbedChunkJob < ApplicationJob
  queue_as :default
  queue_with_priority BACKGROUND_PRIORITY

  retry_on(*Ai::Adapters::Base::TRANSIENT_ERRORS, wait: :polynomially_longer, attempts: 5)

  def perform(chunk)
    entry = chunk.workspace.embedding_model_entry

    # Skip when the chunk already carries a fresh vector for the workspace's
    # current model. stamp_matches? treats a NULL stamp as matching the default
    # entry, so legacy prod rows (embedded before per-model stamping) are
    # correctly skipped for the default workspace.
    return if chunk.stamp_matches?(entry) &&
              chunk.embedding_vector(:embedding, entry.dimensions).present?

    Current.set(workspace: chunk.workspace) do
      vector = EmbeddingService.embed(chunk.content, workspace: chunk.workspace, entry: entry)
      next unless vector

      chunk.assign_embedding(:embedding, vector, entry: entry)
      chunk.save!

      # Enqueue finalization once every chunk for this searchable is fresh
      # for the current entry (not just non-null — a switched model means old
      # 1536 vectors on a mistral workspace are stale and must not count).
      searchable_type = chunk.searchable_type
      searchable_id   = chunk.searchable_id

      total    = SearchChunk.for_searchable(searchable_type, searchable_id).count
      embedded = SearchChunk.for_searchable(searchable_type, searchable_id)
                            .fresh_for(entry)
                            .count

      FinalizeSearchRecordJob.perform_later(searchable_type, searchable_id) if total == embedded
    end
  end
end
