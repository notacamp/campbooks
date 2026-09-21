# frozen_string_literal: true

module Api
  module App
    module Files
      # File upload management for the SPA. Mirrors Files::UploadsController.
      class UploadsController < Api::App::BaseController
        # POST /api/app/files/uploads
        # Accepts: files[] (multipart), folder_id (optional), analyze (boolean)
        def create
          files    = Array(params[:files]).reject(&:blank?)
          folder   = workspace_folder(params[:folder_id])
          analyze  = params[:analyze].present? && params[:analyze].to_s != "false"

          return render_error("no_files", "No files provided.", status: :unprocessable_entity) if files.empty?

          documents = files.map { |file| store_file(file, folder, analyze: analyze) }

          render_data(
            documents.map { |d| Api::App::DocumentSerializer.new(d).as_json },
            status: :created
          )
        end

        # DELETE /api/app/files/uploads/:id
        def destroy
          document = current_workspace.documents.manual_upload.find(params[:id])
          unless document.destroy
            return render_error("cannot_delete", "Cannot delete a statement document.",
                                status: :unprocessable_entity)
          end
          render json: {}, status: :no_content
        end

        # POST /api/app/files/uploads/:id/analyze
        def analyze
          document = current_workspace.documents.manual_upload.find(params[:id])
          document.update!(ai_status: :pending, review_status: :pending,
                           ai_processing_attempts: 0, ai_error: nil)
          DocumentProcessJob.perform_later(document.id)
          render_data(Api::App::DocumentSerializer.new(document).as_json)
        end

        private

        def store_file(file, folder, analyze: false)
          document = ::Document.new(
            source: :manual_upload,
            ai_status:     analyze ? :pending : :skipped,
            review_status: analyze ? :pending : :approved,
            document_type: :other,
            workspace: current_workspace
          )
          document.original_file.attach(file)
          document.save!
          DocumentProcessJob.perform_later(document.id) if analyze

          if folder
            document.folder_memberships.find_or_create_by!(mail_folder: folder)
          end
          document
        end

        def workspace_folder(id)
          return nil if id.blank?
          current_workspace.mail_folders.find_by(id: id)
        end
      end
    end
  end
end
