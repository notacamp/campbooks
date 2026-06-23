module Documents
  # Interactive "Send this document to Google Drive": browse folders, optionally
  # create one, then upload the document's file. Full Drive scope is required to
  # list folders (see GoogleDriveAccount#full_access?).
  class DriveExportsController < ApplicationController
    before_action :set_document
    before_action :set_account

    # GET /documents/:document_id/drive_export/new?parent_id=
    def new
      @parent_id = params[:parent_id].presence
      @current = (@parent_id && @parent_id != "root") ? client.get_folder(@parent_id) : nil
      @up_id = @current&.parents&.first || "root"
      @folders = client.list_folders(parent_id: @parent_id)
    rescue ::GoogleDrive::ApiError, Faraday::Error => e
      redirect_to @document, error: t(".browse_failed", message: e.message)
    end

    # POST /documents/:document_id/drive_export/create_folder
    def create_folder
      name = params[:name].to_s.strip
      parent_id = params[:parent_id].presence
      if name.blank?
        return redirect_to new_document_drive_export_path(@document, parent_id: parent_id),
                           error: t(".folder_name_required")
      end

      folder = Integrations::Drive::FolderCreator.new(@account).call(name: name, parent_id: parent_id)
      # Drop the user inside the folder they just made.
      redirect_to new_document_drive_export_path(@document, parent_id: folder.id),
                  success: t(".folder_created", name: folder.name)
    rescue => e
      redirect_to new_document_drive_export_path(@document, parent_id: parent_id),
                  error: t(".folder_failed", message: e.message)
    end

    # POST /documents/:document_id/drive_export — upload into the chosen folder
    def create
      folder_id = params[:folder_id].presence
      folder_id = nil if folder_id == "root"

      files = Integrations::FileSource.for(document: @document)
      return redirect_to @document, error: t(".no_file") if files.empty?

      Integrations::Drive::FileUploader.new(@account).call(files: files, folder_id: folder_id)
      redirect_to @document, success: t(".uploaded")
    rescue => e
      redirect_to @document, error: t(".upload_failed", message: e.message)
    end

    private

    def set_document
      @document = Current.workspace.documents.find(params[:document_id])
    end

    def set_account
      @account = Current.workspace.google_drive_accounts.connected.first
      unless @account
        return redirect_to settings_integrations_google_drive_path,
                           error: t("documents.drive_exports.errors.not_connected")
      end
      unless @account.full_access?
        redirect_to settings_integrations_google_drive_path,
                    error: t("documents.drive_exports.errors.reconnect_needed")
      end
    end

    def client
      @client ||= ::GoogleDrive::Client.new(@account)
    end
  end
end
