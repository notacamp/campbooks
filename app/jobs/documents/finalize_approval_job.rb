# frozen_string_literal: true

module Documents
  # Runs after a human approves a document — immediately for the detail/list approve,
  # and ~7s after Skim approve (so an Undo, which flips review_status back to :pending,
  # can cancel the push before it fires). Re-checks the doc is *still* approved, then
  # runs the post-approval drive pushes: Google Drive (when the document type opts into
  # auto_push) and Zoho Drive (when a DriveFolderMapping opts into auto_sync). The manual
  # push_to_* controller actions stay independent of this.
  class FinalizeApprovalJob < ApplicationJob
    queue_as :default

    def perform(document_id)
      document = Document.find_by(id: document_id)
      return unless document&.review_approved? # Undo reverted it within the window — bail.

      push_to_google_drive(document)
      push_to_zoho_drive(document)
    end

    private

    def push_to_google_drive(document)
      return unless document.classification&.google_drive_config&.auto_push?
      return if document.pushed_to_drive?

      GoogleDrivePushJob.perform_later(document.id)
    end

    def push_to_zoho_drive(document)
      mapping = DriveFolderMapping.find_by(document_type_id: document.document_type_id)
      return unless mapping&.auto_sync?
      return unless mapping.zoho_drive_account&.active?

      ZohoDriveUploadJob.perform_later(document.id)
    end
  end
end
