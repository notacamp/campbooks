module Files
  # File upload for the Files area. Stores each file as a Document. The upload form's
  # "Analyze with AI" toggle decides whether it enters the (invoice/receipt) AI
  # pipeline immediately (`analyze: true` → ai_status: :pending + DocumentProcessJob,
  # mirroring DocumentsController#create) or is stored as-is (`ai_status: :skipped` —
  # a plain file isn't necessarily a business document). A skipped file can still be
  # sent through the pipeline later via #analyze.
  class UploadsController < ApplicationController
    # Only the user's own uploads are deletable/analyzable from here — never an
    # email-sourced Document, which belongs to the mail pipeline.
    def create
      files = Array(params[:files]).reject(&:blank?)
      folder = workspace_folder(params[:folder_id])
      analyze = params[:analyze].present?

      if files.empty?
        redirect_to(folder ? files_folder_path(folder) : files_path, error: t(".no_files"))
        return
      end

      documents = files.map { |file| store(file, folder, analyze: analyze) }
      notify_all_users(documents) if analyze

      target = folder ? files_folder_path(folder) : files_path
      redirect_to target, success: upload_message(documents.size, analyze)
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

    def store(file, folder, analyze: false)
      document = Document.new(
        source: :manual_upload,
        ai_status: analyze ? :pending : :skipped,
        review_status: analyze ? :pending : :approved,
        document_type: :other,
        workspace: Current.workspace
      )
      document.original_file.attach(file)
      document.save!
      DocumentProcessJob.perform_later(document.id) if analyze
      Events.publish("file.uploaded", subject: document,
        payload: { "filename" => document.original_file.filename.to_s, "analyzed" => analyze })

      if folder
        document.folder_memberships.find_or_create_by!(mail_folder: folder)
        Events.publish("file.filed", subject: document,
          payload: { "filename" => document.original_file.filename.to_s, "folder" => folder.name })
      end
      document
    end

    # Absolute keys (not lazy `t(".x")`) — this helper is called from #create, but a
    # lazy key would scope to "…uploads.upload_message.*" (the method), not create.
    def upload_message(count, analyze)
      scope = "files.uploads.create"
      if analyze
        count == 1 ? t("#{scope}.analyzed_one") : t("#{scope}.analyzed_many", count: count)
      else
        count == 1 ? t("#{scope}.uploaded_one") : t("#{scope}.uploaded_many", count: count)
      end
    end

    # Quiet team-activity notification when files enter the AI pipeline (mirrors
    # DocumentsController#notify_all_users). Plain stored files stay silent — the
    # `file.uploaded` event already records them on the activity timeline.
    def notify_all_users(documents)
      count = documents.size
      label = count == 1 ? documents.first.original_file.filename.to_s : "#{count} documents"
      Current.workspace.users.find_each do |user|
        next if user == current_user

        Notification.notify(
          user: user,
          category: :activity,
          priority: :activity, # quiet team-activity tier — no toast
          title: "New document uploaded",
          body: "#{current_user.name} uploaded #{label}",
          link_url: count == 1 ? document_path(documents.first) : files_path,
          group_key: "manual_upload",
          respect_preferences: false
        )
      end
    end

    def workspace_folder(id)
      return nil if id.blank?
      Current.workspace.mail_folders.find_by(id: id)
    end
  end
end
