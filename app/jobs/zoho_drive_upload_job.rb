class ZohoDriveUploadJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(document_id)
    document = Document.find(document_id)
    return unless document.review_approved?

    mapping = DriveFolderMapping.find_by(document_type_id: document.document_type_id)
    unless mapping
      # Try catch-all mapping (nil document_type_id)
      mapping = DriveFolderMapping.find_by(document_type_id: nil, zoho_drive_account: ZohoDriveAccount.active.first)
    end
    return unless mapping

    account = mapping.zoho_drive_account
    return unless account.active?

    upload = DocumentDriveUpload.find_or_initialize_by(
      document: document,
      zoho_drive_account: account
    )
    return if upload.status == "uploaded"

    upload.update!(status: "pending")

    client = Zoho::DriveClient.new(account)
    file_to_upload = document.processed_pdf.attached? ? document.processed_pdf : document.original_file

    Tempfile.create([ "campbooks-drive-", File.extname(file_to_upload.filename.to_s) ]) do |tmp|
      tmp.binmode
      tmp.write(file_to_upload.download)
      tmp.flush

      result = client.upload_file(
        file_path: tmp.path,
        filename: document.canonical_filename.presence || file_to_upload.filename.to_s,
        parent_id: mapping.drive_folder_id
      )

      if result && result["id"]
        upload.update!(
          status: "uploaded",
          drive_file_id: result["id"],
          uploaded_at: Time.current
        )
        account.record_sync!
      else
        upload.update!(status: "failed", error_message: "Upload returned no file ID")
      end
    end
  rescue => e
    Rails.logger.error("[ZohoDriveUploadJob] Upload failed for document #{document_id}: #{e.message}")
    upload&.update!(status: "failed", error_message: e.message)
    raise
  end
end
