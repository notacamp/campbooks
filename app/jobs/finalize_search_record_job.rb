class FinalizeSearchRecordJob < ApplicationJob
  queue_as :default

  def perform(searchable_type, searchable_id)
    searchable = searchable_type.constantize.find_by(id: searchable_id)
    return unless searchable

    workspace = searchable.searchable_workspace

    chunks = SearchChunk.for_searchable(searchable_type, searchable_id)
                        .where.not(embedding: nil)
                        .order(:position)

    return if chunks.empty?

    # Compute content embedding as weighted average of chunk embeddings
    content_embedding = compute_weighted_average(chunks)

    # Generate title embedding
    title = searchable.searchable_title
    title_embedding = if title.present?
      EmbeddingService.embed(title, workspace: workspace)
    end

    # Upsert the search record
    search_record = SearchRecord.find_or_initialize_by(
      searchable_type: searchable_type,
      searchable_id: searchable_id
    )

    search_record.update!(
      workspace: workspace,
      title: title,
      content_preview: searchable.searchable_content_preview,
      tags: searchable.searchable_tags,
      filter_data: searchable.searchable_filter_data,
      title_embedding: title_embedding,
      content_embedding: content_embedding,
      embedding_model: EmbeddingService::DEFAULT_MODEL,
      source_created_at: searchable.searchable_source_created_at,
      source_updated_at: searchable.searchable_source_updated_at,
      indexed_at: Time.current
    )
  end

  private

  def compute_weighted_average(chunks)
    total_weight = chunks.sum { |c| (c.token_count || 1).to_f }
    return nil if total_weight.zero?

    dimension = EmbeddingService::DIMENSION
    weighted_sum = Array.new(dimension, 0.0)

    chunks.each do |chunk|
      weight = (chunk.token_count || 1).to_f / total_weight
      embedding = chunk.embedding
      next unless embedding

      embedding.each_with_index do |val, i|
        weighted_sum[i] += val * weight
      end
    end

    weighted_sum
  end
end
