# frozen_string_literal: true

module Api
  module App
    module Compose
      # File attachment upload for the React composer. The SPA uploads each file
      # and stores the returned signed_id as a hidden field on the compose form;
      # Emails::Sender resolves those ids into bytes at send time. Mirrors
      # ComposeAttachmentsController but JSON-native.
      class AttachmentsController < BaseController
        MAX_BYTES = 25.megabytes

        def create
          file = params[:file]
          unless file.respond_to?(:tempfile)
            return render_error("invalid_file", "Expected a multipart file upload.", status: :bad_request)
          end
          if file.size.to_i > MAX_BYTES
            return render_error("too_large", "Attachment must be under 25 MB.", status: :unprocessable_entity)
          end

          blob = ::ActiveStorage::Blob.create_and_upload!(
            io: file.tempfile,
            filename: file.original_filename.presence || "attachment",
            content_type: file.content_type.presence || "application/octet-stream"
          )
          current_user.outbound_attachments.attach(blob)

          render_data({
            signed_id: blob.signed_id,
            filename: blob.filename.to_s,
            size: blob.byte_size,
            content_type: blob.content_type
          }, status: :created)
        rescue => e
          Rails.logger.error("[Api::App::Compose::AttachmentsController] upload failed: #{e.class}: #{e.message}")
          render_error("upload_failed", "Attachment upload failed.", status: :unprocessable_entity)
        end
      end
    end
  end
end
