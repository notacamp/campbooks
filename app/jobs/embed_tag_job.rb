class EmbedTagJob < ApplicationJob
  queue_as :default
  queue_with_priority BACKGROUND_PRIORITY

  def perform(tag)
    return unless tag.workspace.present?
    return unless Ai::ProviderSetup.configured?(tag.workspace, :embeddings)

    entry        = tag.workspace.embedding_model_entry
    content      = SearchTagEmbedding.embedding_text_for(tag)
    content_hash = Digest::SHA256.hexdigest(content)

    existing = SearchTagEmbedding.find_by(tag_id: tag.id)

    # Skip only when the content is unchanged AND the row is already fresh for
    # the workspace's current model — a model switch must re-embed even when the
    # tag name/prompt has not changed.
    return if existing &&
              existing.content_hash == content_hash &&
              existing.stamp_matches?(entry) &&
              existing.embedding_vector(:embedding, entry.dimensions).present?

    vector = EmbeddingService.embed(content, workspace: tag.workspace, entry: entry)
    return unless vector

    search_embedding = SearchTagEmbedding.find_or_initialize_by(tag_id: tag.id)
    search_embedding.workspace    = tag.workspace
    search_embedding.content_hash = content_hash
    search_embedding.assign_embedding(:embedding, vector, entry: entry)
    search_embedding.save!
  end
end
