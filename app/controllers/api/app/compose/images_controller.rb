# frozen_string_literal: true

module Api
  module App
    module Compose
      # Inline-image upload for the React compose editor (paste, drop, file pick).
      # Mirrors ComposeImagesController but returns JSON only. We store the blob
      # attached to the uploader and return a stable rails_storage_proxy_url so
      # the image survives in sent mail even after the original attachment expires.
      class ImagesController < BaseController
        ALLOWED_TYPES = %w[image/png image/jpeg image/gif image/webp].freeze
        MAX_BYTES = 10.megabytes

        def create
          file = params[:image]
          unless file.respond_to?(:tempfile) && file.respond_to?(:content_type)
            return render_error("invalid_file", "Expected a multipart image upload.", status: :bad_request)
          end
          unless ALLOWED_TYPES.include?(file.content_type)
            return render_error("unsupported_type",
                                "Only PNG, JPEG, GIF and WebP images are supported.",
                                status: :unprocessable_entity)
          end
          if file.size.to_i > MAX_BYTES
            return render_error("too_large", "Image must be under 10 MB.", status: :unprocessable_entity)
          end

          blob = ::ActiveStorage::Blob.create_and_upload!(
            io: file.tempfile,
            filename: file.original_filename.presence || "image",
            content_type: file.content_type
          )
          current_user.outbound_images.attach(blob)

          render_data({
            url: rails_storage_proxy_url(blob),
            alt: ::File.basename(file.original_filename.to_s, ".*")
          }, status: :created)
        rescue => e
          Rails.logger.error("[Api::App::Compose::ImagesController] upload failed: #{e.class}: #{e.message}")
          render_error("upload_failed", "Image upload failed.", status: :unprocessable_entity)
        end
      end
    end
  end
end
