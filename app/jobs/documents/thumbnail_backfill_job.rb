# frozen_string_literal: true

module Documents
  # One-shot sweep that renders thumbnails for documents created before the
  # Files grid view shipped (new documents get theirs from after_create_commit).
  # Enqueued once by a migration; safe to re-run any time — it only touches
  # documents with no thumbnail attached and processes them inline, serially,
  # as ONE job (never a per-document fan-out, which would flood the queue and
  # starve user jobs).
  class ThumbnailBackfillJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    BATCH_SIZE = 100

    def perform
      generated = 0
      scope.find_each(batch_size: BATCH_SIZE) do |document|
        generated += 1 if Documents::ThumbnailGenerator.new(document).call
      end
      Rails.logger.info("[Documents::ThumbnailBackfillJob] generated #{generated} thumbnails")
    end

    private

    # PDFs and images with no thumbnail attachment yet (find_each batches by
    # primary key, so no meaningful ordering is possible here).
    def scope
      thumbed = ActiveStorage::Attachment.where(record_type: "Document", name: "thumbnail").select(:record_id)
      Document.where.not(id: thumbed)
              .joins(original_file_attachment: :blob)
              .where("active_storage_blobs.content_type = 'application/pdf' OR active_storage_blobs.content_type LIKE 'image/%'")
    end
  end
end
