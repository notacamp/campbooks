# frozen_string_literal: true

module Api
  module App
    module Files
      # Files surface — root folder tree + contents. Mirrors FilesController.
      class FilesController < Api::App::BaseController
        include Pagy::Backend

        # GET /api/app/files
        # Root: all folders + all workspace files.
        def index
          folders = current_workspace.mail_folders.accessible_to(current_user).ordered.to_a
          folder_counts = MailFolder.item_counts(folders)

          scope = current_workspace.documents.accessible_to(current_user)
                    .with_attached_original_file.recent
          pagy, docs = pagy(scope, limit: per_page)

          render_data(
            Api::App::FilesFolderSerializer.new(
              folders:       folders,
              documents:     docs,
              pagy:          pagy,
              folder_counts: folder_counts
            ).as_json
          )
        end

        # GET /api/app/files/folders/:id
        def show
          folder = current_workspace.mail_folders.find(params[:id])

          unless folder.readable_by?(current_user)
            return render_not_found
          end

          folders = current_workspace.mail_folders.accessible_to(current_user).ordered.to_a
          folder_counts = MailFolder.item_counts(folders)

          scope = current_workspace.documents.accessible_to(current_user)
                    .joins(:folder_memberships)
                    .where(folder_memberships: { mail_folder_id: folder.id })
                    .with_attached_original_file.recent
          pagy, docs = pagy(scope, limit: per_page)

          render_data(
            Api::App::FilesFolderSerializer.new(
              folders:        folders,
              current_folder: folder,
              documents:      docs,
              pagy:           pagy,
              folder_counts:  folder_counts
            ).as_json
          )
        end
      end
    end
  end
end
