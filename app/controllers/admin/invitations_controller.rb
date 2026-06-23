class Admin::InvitationsController < Admin::BaseController
  def index
    @invitations = Invitation.pending_admin_approval.chronological
  end

  def approve
    invitation = Invitation.pending.find(params[:id])
    invitation.approve_by_admin!
    Notifier.invitation_resolved(invitation)
    redirect_to admin_invitations_path, success: t(".success", email: invitation.email)
  end

  def reject
    invitation = Invitation.pending.find(params[:id])
    invitation.cancel!
    Notifier.invitation_resolved(invitation)
    redirect_to admin_invitations_path, success: t(".success", email: invitation.email)
  end
end
