class FilesController < ApplicationController
  # The Files area — a unified file manager over the workspace's files (Documents)
  # organized into custom folders (MailFolder). #index lists everything ("All
  # files"); #show scopes to a single folder (its subfolders + filed contents).
  # Both render the same template; @folder distinguishes the two.
  def index
    load_folders
    @pagy, @files = pagy(files_scope, items: 30)
    # Show the file-manager layout (sidebar + list) once there's anything to
    # organize — files OR folders. A brand-new, empty area gets the upload CTA.
    @has_any_files = @folders.any? || Current.workspace.documents.exists?

    respond_to do |format|
      format.html
      format.turbo_stream
    end
  end

  def show
    @folder = Current.workspace.mail_folders.find(params[:id])
    @subfolders = @folder.children.to_a
    load_folders
    @pagy, @files = pagy(files_scope, items: 30)
    @has_any_files = true

    respond_to do |format|
      format.html { render :index }
      format.turbo_stream { render :index }
    end
  end

  private

  def load_folders
    @folders = Current.workspace.mail_folders.ordered.to_a
    @folder_counts = MailFolder.document_counts(@folders)
  end

  # "All files" by default; a single folder's contents when scoped (via #show, or
  # the ?folder_id= filter the mobile picker uses). Mirrors DocumentsController's
  # includes/order so rows render without N+1s.
  def files_scope
    scope = if @folder
      @folder.documents
    elsif params[:folder_id].present?
      Current.workspace.documents.in_folder(params[:folder_id])
    else
      Current.workspace.documents
    end
    scope.includes(:classification).with_attached_original_file.starred_first.recent
  end
end
