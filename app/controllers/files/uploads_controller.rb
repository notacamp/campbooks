module Files
  # Light-path file upload for the Files area. Stores each file as a Document but
  # skips the (invoice/receipt) AI pipeline — a plain file isn't necessarily a
  # business document — by creating it `ai_status: :skipped` and never enqueuing
  # DocumentProcessJob. The user can opt a file into analysis later via #analyze,
  # which flips it back to :pending and enqueues the job (mirrors #reprocess).
  class UploadsController < ApplicationController
    # Only the user's own uploads are deletable/analyzable from here — never an
    # email-sourced Document, which belongs to the mail pipeline.
    def create
      files = Array(params[:files]).reject(&:blank?)
      folder = workspace_folder(params[:folder_id])

      if files.empty?
        redirect_to(folder ? files_folder_path(folder) : files_path, error: t(".no_files"))
        return
      end

      documents = files.map { |file| store(file, folder) }

      target = folder ? files_folder_path(folder) : files_path
      message = documents.size == 1 ? t(".uploaded_one") : t(".uploaded_many", count: documents.size)
      redirect_to target, success: message
    rescue => e
      redirect_to(files_path, error: t(".failed", message: e.message))
    end

    def destroy
      document = Current.workspace.documents.manual_upload.find(params[:id])
      filename = document.original_file.filename.to_s if document.original_file.attached?
      document.destroy
      Events.publish("file.deleted", payload: { "filename" => filename })
      redirect_back fallback_location: files_path, success: t(".deleted")
    end

    # Send a stored file through the AI pipeline on demand (classification + extraction).
    def analyze
      return if require_ai_provider!(:documents)

      document = Current.workspace.documents.manual_upload.find(params[:id])
      document.update!(ai_status: :pending, review_status: :pending,
                       ai_processing_attempts: 0, ai_error: nil)
      DocumentProcessJob.perform_later(document.id)
      redirect_back fallback_location: files_path, success: t(".analyzing")
    end

    private

    def store(file, folder)
      document = Document.new(
        source: :manual_upload,
        ai_status: :skipped,
        review_status: :approved,
        document_type: :other,
        workspace: Current.workspace
      )
      document.original_file.attach(file)
      document.save!
      Events.publish("file.uploaded", subject: document,
        payload: { "filename" => document.original_file.filename.to_s, "analyzed" => false })

      if folder
        document.folder_memberships.find_or_create_by!(mail_folder: folder)
        Events.publish("file.filed", subject: document,
          payload: { "filename" => document.original_file.filename.to_s, "folder" => folder.name })
      end
      document
    end

    def workspace_folder(id)
      return nil if id.blank?
      Current.workspace.mail_folders.find_by(id: id)
    end
  end
end
