class ReindexSearchableJob < ApplicationJob
  queue_as :default

  def perform(searchable)
    return if searchable.search_record&.indexed_at && searchable.search_record.indexed_at > 5.minutes.ago

    searchable.search_chunks.destroy_all
    searchable.search_record&.destroy!
    EmbedSearchableJob.perform_later(searchable)
  end
end
