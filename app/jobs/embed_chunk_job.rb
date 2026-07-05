class EmbedChunkJob < ApplicationJob
  queue_as :default

  def perform(chunk)
    return if chunk.embedding.present? && chunk.embedding_model.present?

    Current.set(workspace: chunk.workspace) do
      vector = EmbeddingService.embed(chunk.content, workspace: chunk.workspace)
      next unless vector

      chunk.update!(
        embedding: vector,
        embedding_model: EmbeddingService::DEFAULT_MODEL
      )

      # Check if all chunks for this searchable are now embedded
      searchable_type = chunk.searchable_type
      searchable_id = chunk.searchable_id

      total = SearchChunk.for_searchable(searchable_type, searchable_id).count
      embedded = SearchChunk.for_searchable(searchable_type, searchable_id)
                            .where.not(embedding: nil)
                            .count

      FinalizeSearchRecordJob.perform_later(searchable_type, searchable_id) if total == embedded
    end
  end
end
