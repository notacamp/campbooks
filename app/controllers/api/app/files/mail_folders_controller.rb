# frozen_string_literal: true

module Api
  module App
    module Files
      # Mail folder (inbox label folder) CRUD for the SPA. Mirrors MailFoldersController.
      class MailFoldersController < Api::App::BaseController
        before_action :set_folder, only: %i[show update destroy]

        # GET /api/app/mail_folders/:id
        def show
          return render_not_found unless @folder.readable_by?(current_user)
          render_data(folder_as_json(@folder))
        end

        # POST /api/app/mail_folders
        def create
          folder = current_workspace.mail_folders.new(
            mail_folder_params.merge(position: MailFolder.next_position_for(current_workspace))
          )
          if folder.save
            render_data(folder_as_json(folder), status: :created)
          else
            render_record_invalid(ActiveRecord::RecordInvalid.new(folder))
          end
        end

        # PATCH /api/app/mail_folders/:id
        def update
          if @folder.update(mail_folder_params)
            render_data(folder_as_json(@folder))
          else
            render_record_invalid(ActiveRecord::RecordInvalid.new(@folder))
          end
        end

        # DELETE /api/app/mail_folders/:id
        def destroy
          @folder.destroy
          render json: {}, status: :no_content
        end

        private

        def set_folder
          @folder = current_workspace.mail_folders.find(params[:id])
        end

        def mail_folder_params
          params.require(:mail_folder).permit(:name, :icon, :parent_id, :position)
        end

        def folder_as_json(folder)
          {
            id:        folder.id,
            name:      folder.name,
            icon:      folder.try(:icon),
            parent_id: folder.parent_id,
            position:  folder.position,
            workspace_id: folder.workspace_id
          }
        end
      end
    end
  end
end
