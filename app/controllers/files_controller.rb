class FilesController < ApplicationController
  # The Files area — a unified file manager over the workspace's files (Documents)
  # organized into custom folders (MailFolder). #index lists everything ("All
  # files"); #show scopes to a single folder (its subfolders + filed contents).
  # Both render the same template; @folder distinguishes the two.
  def index
    load_folders
    # "All files": the workspace's internal documents + uploaded files. Emails only
    # surface inside a folder they've been filed into (see #show).
    @internal_docs = Current.workspace.authored_documents.accessible_to(Current.user).recent.limit(50).to_a
    @filed_emails = []
    @pagy, @files = pagy(files_scope, items: 30)
    # Show the file-manager layout (sidebar + list) once there's anything to
    # organize — files, internal docs, or folders. A brand-new area gets the CTA.
    @has_any_files = @folders.any? || @internal_docs.any? || Current.workspace.documents.exists?

    respond_to do |format|
      format.html
      format.turbo_stream
    end
  end

  def show
    @folder = Current.workspace.mail_folders.find(params[:id])
    # 404 (not 403) for a restricted folder the user can't read — don't leak existence.
    return head :not_found unless @folder.readable_by?(Current.user)

    @subfolders = @folder.children.to_a
    load_folders
    @internal_docs = @folder.authored_documents.recent.limit(50).to_a
    @filed_emails = @folder.email_messages.accessible_to(Current.user).order(received_at: :desc).limit(50).to_a
    @pagy, @files = pagy(files_scope, items: 30)
    @has_any_files = true

    respond_to do |format|
      format.html { render :index }
      format.turbo_stream { render :index }
    end
  end

  private

  def load_folders
    @folders = Current.workspace.mail_folders.accessible_to(Current.user).ordered.to_a
    # Badge counts every filed item kind (files + internal docs + emails).
    @folder_counts = MailFolder.item_counts(@folders)
  end

  # "All files" by default; a single folder's contents when scoped (via #show, or
  # the ?folder_id= filter the mobile picker uses). Mirrors DocumentsController's
  # includes/order so rows render without N+1s.
  def files_scope
    scope = if @folder
      @folder.documents
    elsif params[:folder_id].present?
      Current.workspace.documents.accessible_to(Current.user).in_folder(params[:folder_id])
    else
      Current.workspace.documents.accessible_to(Current.user)
    end
    scope.includes(:classification).with_attached_original_file.starred_first.recent
  end
end
