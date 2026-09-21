# frozen_string_literal: true

module Api
  module App
    module Files
      # Folder membership (filing) for the SPA. Mirrors FolderMembershipsController.
      class FolderMembershipsController < Api::App::BaseController
        ALLOWED_FOLDERABLE_TYPES = %w[Document AuthoredDocument EmailMessage].freeze

        # POST /api/app/folder_memberships
        def create
          folder     = current_workspace.mail_folders.find(params[:mail_folder_id])
          folderable = resolve_folderable!

          membership = folder.folder_memberships.find_or_create_by!(folderable: folderable)
          render_data({ id: membership.id, mail_folder_id: folder.id,
                        folderable_type: folderable.class.name, folderable_id: folderable.id },
                      status: :created)
        end

        # DELETE /api/app/folder_memberships/:id
        def destroy
          membership = FolderMembership.joins(:mail_folder)
                                       .where(mail_folders: { workspace_id: current_workspace.id })
                                       .find(params[:id])
          membership.destroy
          render json: {}, status: :no_content
        end

        private

        def resolve_folderable!
          type = params[:folderable_type].presence || "Document"
          raise ActiveRecord::RecordNotFound unless ALLOWED_FOLDERABLE_TYPES.include?(type)

          case type
          when "Document"
            current_workspace.documents.find(params[:folderable_id])
          when "AuthoredDocument"
            current_workspace.authored_documents.find(params[:folderable_id])
          when "EmailMessage"
            EmailMessage.accessible_to(current_user).find(params[:folderable_id])
          end
        end
      end
    end
  end
end
