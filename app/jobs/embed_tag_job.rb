class EmbedTagJob < ApplicationJob
  queue_as :default

  def perform(tag)
    return unless tag.workspace.present?
    return unless Ai::ProviderSetup.configured?(tag.workspace, :embeddings)

    content = build_tag_content(tag)
    content_hash = Digest::SHA256.hexdigest(content)

    existing = SearchTagEmbedding.find_by(tag_id: tag.id)
    return if existing && existing.content_hash == content_hash

    vector = EmbeddingService.embed(content, workspace: tag.workspace)
    return unless vector

    search_embedding = SearchTagEmbedding.find_or_initialize_by(tag_id: tag.id)
    search_embedding.update!(
      workspace: tag.workspace,
      embedding: vector,
      embedding_model: EmbeddingService::DEFAULT_MODEL,
      content_hash: content_hash
    )
  end

  private

  def build_tag_content(tag)
    parts = [ tag.name ]
    # Tag#prompt already returns plain text (it's overridden on the model), so
    # use it directly — calling .to_plain_text on the String raises.
    parts << tag.prompt if tag.respond_to?(:prompt) && tag.prompt.present?
    parts.join(" -- ")
  end
end
