# frozen_string_literal: true

module Api
  module V1
    # Files a document into (or removes it from) a workspace custom folder.
    # Only Documents are supported as fileable content; folderable_type is always
    # "Document". Both the folder and the document are workspace-scoped so
    # cross-workspace filing is impossible.
    class FolderMembershipsController < BaseController
      before_action -> { doorkeeper_authorize! :"folders:write" }, only: [ :create, :destroy ]

      def create
        folder   = Current.workspace.mail_folders.find(params[:mail_folder_id])
        document = Current.workspace.documents.find(params[:document_id])
        membership = folder.folder_memberships.find_or_create_by!(folderable: document)

        render_data(
          { id: membership.id, folder_id: folder.id, document_id: document.id },
          status: :created
        )
      end

      def destroy
        membership = FolderMembership.joins(:mail_folder)
                                     .where(mail_folders: { workspace_id: Current.workspace.id })
                                     .find(params[:id])
        membership.destroy
        head :no_content
      end
    end
  end
end
