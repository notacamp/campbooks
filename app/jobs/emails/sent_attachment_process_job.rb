# frozen_string_literal: true

module Emails
  class SentAttachmentProcessJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    def perform(email_message_id, user_id, signed_blob_ids)
      email = EmailMessage.find(email_message_id)
      user = User.find(user_id)
      Current.workspace = email.email_account.workspace
      owned_blob_ids = user.outbound_attachments.blobs.pluck(:id).to_set
      signed_blob_ids.each { |sid| process_blob(sid, owned_blob_ids, user, email) }
    rescue => e
      Rails.logger.error("[SentAttachmentProcessJob] Error: #{e.message}")
      raise
    ensure
      Current.workspace = nil
    end

    private

    def process_blob(signed_id, owned_blob_ids, user, email)
      blob = ActiveStorage::Blob.find_signed(signed_id)
      return unless blob && owned_blob_ids.include?(blob.id)
      raw_data = blob.download
      return if raw_data.nil? || raw_data.empty?
      return if tracking_image?(raw_data, blob.content_type)
      email.files.attach(io: StringIO.new(raw_data), filename: blob.filename.to_s, content_type: blob.content_type)
      create_document(email, blob, raw_data)
      user.outbound_attachments.find_by(blob_id: blob.id)&.purge_later
    rescue => e
      Rails.logger.error("[SentAttachmentProcessJob] Blob error: #{e.message}")
    end

    def create_document(email, blob, raw_data)
      content_hash = Digest::SHA256.hexdigest(raw_data)
      existing = Document.find_by(content_hash: content_hash, email_account_id: email.email_account_id)
      if existing
        existing.document_email_messages.find_or_create_by!(email_message: email)
      else
        document = Document.new(source: :sent_email, ai_status: :pending, review_status: :pending,
          document_type: :other, workspace: email.email_account.workspace, email_account: email.email_account,
          email_message_id: email.provider_message_id, content_hash: content_hash)
        document.original_file.attach(io: StringIO.new(raw_data), filename: blob.filename.to_s, content_type: blob.content_type)
        document.save!
        document.document_email_messages.create!(email_message: email)
        DocumentProcessJob.perform_later(document.id)
      end
    end

    TRACKING_IMAGE_MAX_DIMENSION = 32
    def tracking_image?(raw_data, content_type)
      return false unless content_type.to_s.start_with?("image/")
      width, height = Images::Dimensions.read(raw_data)
      if width && height
        width <= TRACKING_IMAGE_MAX_DIMENSION && height <= TRACKING_IMAGE_MAX_DIMENSION
      else
        raw_data.bytesize <= 1024
      end
    end
  end
end
