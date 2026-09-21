# frozen_string_literal: true

module Api
  module App
    module Files
      # Public share-link management for Files. Mirrors Files::PublicLinksController.
      class PublicLinksController < Api::App::BaseController
        # POST /api/app/files/public_links
        def create
          shareable = locate_shareable
          return render_not_found unless shareable

          link = FileShareLink.active.find_by(shareable: shareable) ||
            FileShareLink.create!(shareable: shareable, created_by: current_user,
                                  workspace: current_workspace)

          render_data({
            id:    link.id,
            token: link.token,
            url:   link.public_url(host: request.base_url),
            name:  share_label(shareable)
          }, status: :created)
        end

        # DELETE /api/app/files/public_links/:id
        def destroy
          link = FileShareLink.where(workspace: current_workspace).find(params[:id])
          link.revoke!
          render json: {}, status: :no_content
        end

        # GET /api/app/files/public_links/picker
        def picker
          documents = current_workspace.documents.accessible_to(current_user).recent.limit(40).to_a
          render_data(documents.map { |d| Api::App::DocumentSerializer.new(d).as_json })
        end

        private

        def locate_shareable
          case params[:shareable_type]
          when "Document"
            current_workspace.documents.accessible_to(current_user).find_by(id: params[:shareable_id])
          when "AuthoredDocument"
            current_workspace.authored_documents.accessible_to(current_user).find_by(id: params[:shareable_id])
          end
        end

        def share_label(shareable)
          shareable.try(:display_title) || shareable.try(:title) || shareable.to_s
        end
      end
    end
  end
end
