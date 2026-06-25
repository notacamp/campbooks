class FolderMembershipsController < ApplicationController
  before_action :require_authentication

  # File a document into a folder (the Stage 3 "filesystem" layer). Only documents
  # are filable for now, so folderable_type is assumed and the id is resolved
  # workspace-scoped — never trust a client-supplied type/id to reach another model.
  def create
    folder = Current.workspace.mail_folders.find(params[:mail_folder_id])
    document = Current.workspace.documents.find(params[:folderable_id])
    folder.folder_memberships.find_or_create_by!(folderable: document)

    respond_to do |format|
      format.turbo_stream { render turbo_stream: folders_stream(document) }
      format.html { redirect_back fallback_location: document_path(document) }
    end
  end

  def destroy
    membership = FolderMembership.joins(:mail_folder)
                                 .where(mail_folders: { workspace_id: Current.workspace.id })
                                 .find(params[:id])
    document = membership.folderable
    membership.destroy

    respond_to do |format|
      format.turbo_stream { render turbo_stream: folders_stream(document) }
      format.html { redirect_back fallback_location: document_path(document) }
    end
  end

  private

  def folders_stream(document)
    turbo_stream.replace(helpers.dom_id(document, :folders),
      partial: "documents/folders", locals: { document: document })
  end
end
