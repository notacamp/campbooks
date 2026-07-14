# frozen_string_literal: true

module Documents
  # Renders one document's grid-view thumbnail (see ThumbnailGenerator).
  # Enqueued from Document#after_create_commit; idempotent, so retries and
  # overlap with the backfill sweep are harmless. Render failures are handled
  # inside the generator (icon fallback), so only infrastructure errors retry.
  class GenerateThumbnailJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3
    discard_on ActiveRecord::RecordNotFound

    def perform(document_id)
      Documents::ThumbnailGenerator.new(Document.find(document_id)).call
    end
  end
end
