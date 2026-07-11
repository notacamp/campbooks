class EmbedSearchableJob < ApplicationJob
  queue_as :default
  queue_with_priority BACKGROUND_PRIORITY

  def perform(searchable)
    workspace = searchable.searchable_workspace

    # Until the workspace has set up an embedding provider → don't index. Strict
    # gate so a freshly-synced inbox isn't embedded on the shared platform key
    # before the user opts into AI.
    return unless Ai::ProviderSetup.configured?(workspace, :embeddings)

    # Build chunks from the searchable model
    chunk_data = searchable.build_search_chunks
    return if chunk_data.blank?

    chunk_records = chunk_data.map.with_index do |data, idx|
      searchable.search_chunks.create!(
        workspace: workspace,
        content: data[:content],
        chunk_type: data[:chunk_type] || "text",
        position: data[:position] || idx,
        token_count: data[:token_count] || estimate_tokens(data[:content]),
        metadata: data[:metadata] || {}
      )
    end

    # Enqueue embedding for each chunk
    chunk_records.each do |chunk|
      EmbedChunkJob.perform_later(chunk)
    end
  end

  private

  def estimate_tokens(text)
    return 0 if text.blank?
    (text.length / 3.5).ceil
  end
end
