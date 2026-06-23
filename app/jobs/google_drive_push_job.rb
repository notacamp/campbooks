class GoogleDrivePushJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(document_id)
    document = Document.find(document_id)
    return if document.pushed_to_drive?

    GoogleDrive::Uploader.new(document).call
  end
end
