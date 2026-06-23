# Inline-image upload for the compose / signature rich-text editor. The
# `tiptap-editor` Stimulus controller POSTs an `image` file here (on pick, paste,
# or drop) and inserts the returned URL as an <img>. We store the blob attached
# to the uploader and hand back a stable, app-served *proxy* URL — unlike a
# redirect/S3 URL it never expires, so a recipient opening the email weeks later
# still sees the image.
class ComposeImagesController < ApplicationController
  before_action :require_authentication

  # Raster types only — no SVG (it can carry script, and we serve images inline).
  ALLOWED_TYPES = %w[image/png image/jpeg image/gif image/webp].freeze
  MAX_BYTES = 10.megabytes

  def create
    file = params[:image]
    return render_error(t(".invalid")) unless file.respond_to?(:tempfile) && file.respond_to?(:content_type)
    return render_error(t(".unsupported_type")) unless ALLOWED_TYPES.include?(file.content_type)
    return render_error(t(".too_large")) if file.size.to_i > MAX_BYTES

    blob = ActiveStorage::Blob.create_and_upload!(
      io: file.tempfile,
      filename: file.original_filename.presence || "image",
      content_type: file.content_type
    )
    Current.user.outbound_images.attach(blob)

    render json: {
      url: rails_storage_proxy_url(blob),
      alt: File.basename(file.original_filename.to_s, ".*")
    }
  rescue => e
    Rails.logger.error("[ComposeImages] upload failed: #{e.class}: #{e.message}")
    render_error(t(".failed"))
  end

  private

  def render_error(message)
    render json: { error: message }, status: :unprocessable_entity
  end
end
