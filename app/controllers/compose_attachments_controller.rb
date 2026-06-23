# File-attachment upload for the composer. The `compose-attachments` Stimulus
# controller POSTs each picked file here; we store it (attached to the uploader)
# and return the blob's signed id, which the compose form carries as a hidden
# `attachments[]` field. EmailComposeController#collected_attachments resolves
# those ids back to bytes and hands them to the provider at send time.
class ComposeAttachmentsController < ApplicationController
  before_action :require_authentication

  MAX_BYTES = 25.megabytes

  def create
    file = params[:file]
    return render_error(t(".invalid")) unless file.respond_to?(:tempfile)
    return render_error(t(".too_large")) if file.size.to_i > MAX_BYTES

    blob = ActiveStorage::Blob.create_and_upload!(
      io: file.tempfile,
      filename: file.original_filename.presence || "attachment",
      content_type: file.content_type.presence || "application/octet-stream"
    )
    Current.user.outbound_attachments.attach(blob)

    render json: {
      signed_id: blob.signed_id,
      filename: blob.filename.to_s,
      size: blob.byte_size,
      content_type: blob.content_type
    }
  rescue => e
    Rails.logger.error("[ComposeAttachments] upload failed: #{e.class}: #{e.message}")
    render_error(t(".failed"))
  end

  private

  def render_error(message)
    render json: { error: message }, status: :unprocessable_entity
  end
end
