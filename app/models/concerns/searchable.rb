module Searchable
  extend ActiveSupport::Concern

  included do
    has_one :search_record, as: :searchable, dependent: :destroy
    has_many :search_chunks, as: :searchable, dependent: :destroy

    after_save_commit :enqueue_reindex, if: :searchable_fields_changed?
  end

  # Override in models to define what text gets chunked and embedded.
  # Must return an array of hashes: [{ content:, chunk_type:, position:, metadata: {} }]
  def build_search_chunks
    raise NotImplementedError, "#{self.class} must implement #build_search_chunks"
  end

  def searchable_title
    raise NotImplementedError, "#{self.class} must implement #searchable_title"
  end

  def searchable_content_preview
    raise NotImplementedError, "#{self.class} must implement #searchable_content_preview"
  end

  def searchable_filter_data
    raise NotImplementedError, "#{self.class} must implement #searchable_filter_data"
  end

  def searchable_tags
    raise NotImplementedError, "#{self.class} must implement #searchable_tags"
  end

  def searchable_source_created_at
    respond_to?(:created_at) ? created_at : Time.current
  end

  def searchable_source_updated_at
    respond_to?(:updated_at) ? updated_at : Time.current
  end

  def searchable_workspace
    return workspace if respond_to?(:workspace) && workspace.present?
    raise NotImplementedError, "#{self.class} must implement #searchable_workspace"
  end

  def reindex_search!
    search_chunks.destroy_all
    search_record&.destroy!
    EmbedSearchableJob.perform_later(self)
  end

  private

  def enqueue_reindex
    # Debounce: only reindex if the last reindex was more than 5 minutes ago
    return if search_record&.indexed_at && search_record.indexed_at > 5.minutes.ago

    ReindexSearchableJob.set(wait: 30.seconds).perform_later(self)
  end

  def searchable_fields_changed?
    true # Default: always reindex on save. Override for more granular control.
  end
end
