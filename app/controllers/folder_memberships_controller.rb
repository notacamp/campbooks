class FolderMembershipsController < ApplicationController
  before_action :require_authentication

  # File a document into a folder (the Stage 3 "filesystem" layer). Only documents
  # are filable for now, so folderable_type is assumed and the id is resolved
  # workspace-scoped — never trust a client-supplied type/id to reach another model.
  def create
    folder = Current.workspace.mail_folders.find(params[:mail_folder_id])
    document = Current.workspace.documents.find(params[:folderable_id])
    membership = folder.folder_memberships.find_or_create_by!(folderable: document)
    if membership.previously_new_record?
      Events.publish("file.filed", subject: document,
        payload: { "filename" => document.display_title, "folder" => folder.name })
    end

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
    folder_name = membership.mail_folder.name
    membership.destroy
    Events.publish("file.unfiled", subject: document,
      payload: { "filename" => document.try(:display_title), "folder" => folder_name })

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
