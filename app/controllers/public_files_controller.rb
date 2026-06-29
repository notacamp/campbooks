# Public, unauthenticated file links (Files Phase 3b). Inherits ActionController::Base
# directly (like HealthController) so none of the app's auth / onboarding /
# entitlement before-actions run — the unguessable token IS the credential, so an
# external email recipient can open a shared file without an account. Always 404
# (never 403) for a revoked / expired / deleted target, so a dead link leaks nothing.
class PublicFilesController < ActionController::Base
  def show
    link = FileShareLink.live.find_by(token: params[:token])
    return head(:not_found) unless link&.shareable

    link.record_view!
    Events.publish("file.public_link_viewed", subject: link.shareable, workspace: link.workspace, actor: nil,
      payload: { "type" => link.shareable_type })
    serve(link.shareable)
  end

  private

  def serve(shareable)
    case shareable
    when Document
      blob = shareable.original_file.blob
      return head(:not_found) unless blob

      # The proxy URL is itself capability-based + non-expiring (same mechanism the
      # compose inline-image embeds use), so external recipients can load it.
      redirect_to rails_storage_proxy_url(blob, host: request.base_url), allow_other_host: true
    when AuthoredDocument
      render "public_files/document", layout: "public", locals: { doc: shareable }
    else
      head :not_found
    end
  end
end
