# The user's own security history — sign-ins, MFA changes, password changes, data
# exports, and account-deletion requests recorded by AuditEvent. Strictly per-user
# (the table has no workspace scope); a workspace-wide view would be a separate
# admin-namespaced page.
class Settings::Security::AuditLogController < Settings::BaseController
  def index
    @pagy, @events = pagy(
      AuditEvent.where(user_id: current_user.id).order(created_at: :desc),
      items: 30
    )

    respond_to do |format|
      format.html
      format.turbo_stream # lazy pagination append -> index.turbo_stream.erb
    end
  end
end
