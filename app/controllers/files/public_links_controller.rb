module Files
  # Mint / revoke public links (Files Phase 3b). Authenticated; the public serving
  # endpoint is the separate PublicFilesController. #create returns the link as JSON
  # (for the composer's "insert file link" picker) or redirects back.
  class PublicLinksController < ApplicationController
    def create
      shareable = locate_shareable
      return head(:not_found) unless shareable

      link = FileShareLink.active.find_by(shareable: shareable) ||
        FileShareLink.create!(shareable: shareable, created_by: Current.user, workspace: Current.workspace)
      Events.publish("file.made_public", subject: shareable, payload: { "name" => share_label(shareable) })

      respond_to do |format|
        format.json { render json: { token: link.token, url: link.public_url(host: request.base_url), name: share_label(shareable) } }
        format.html { redirect_back fallback_location: files_path, success: t(".created") }
      end
    end

    def destroy
      link = FileShareLink.where(workspace: Current.workspace).find(params[:id])
      shareable = link.shareable
      link.revoke!
      Events.publish("file.made_private", subject: shareable, payload: { "name" => shareable && share_label(shareable) })
      redirect_back fallback_location: files_path, success: t(".revoked")
    end

    # File list for the composer's "Insert file link" modal (lazy turbo-frame).
    def picker
      @documents = Current.workspace.documents.accessible_to(Current.user).recent.limit(40).to_a
      @authored_documents = Current.workspace.authored_documents.accessible_to(Current.user).recent.limit(40).to_a
      render layout: false
    end

    private

    # Resolve the file to share, scoped to what the user may access.
    def locate_shareable
      case params[:shareable_type]
      when "Document"
        Current.workspace.documents.accessible_to(Current.user).find_by(id: params[:shareable_id])
      when "AuthoredDocument"
        Current.workspace.authored_documents.accessible_to(Current.user).find_by(id: params[:shareable_id])
      end
    end

    def share_label(shareable)
      shareable.try(:display_title) || shareable.try(:title) || shareable.to_s
    end
  end
end
