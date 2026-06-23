class Settings::InvitationsController < Settings::BaseController
  before_action :set_invitation, only: [ :destroy, :resend ]

  def create
    @invitation = Current.workspace.invitations.new(invitation_params)
    @invitation.invited_by = Current.user

    # In cloud mode, non-admin invitations require admin approval
    if !self_hosted? && !Current.user.admin?
      @invitation.admin_approved = false
    end

    if @invitation.save
      if @invitation.admin_approved?
        InvitationMailer.invitation(@invitation).deliver_later
      else
        Notifier.invitation_pending_approval(@invitation)
      end
      if @invitation.admin_approved?
        redirect_to settings_members_path, success: t(".sent", email: @invitation.email)
      else
        redirect_to settings_members_path, success: t(".pending_review", email: @invitation.email)
      end
    else
      redirect_to settings_members_path,
                  error: @invitation.errors.full_messages.to_sentence
    end
  end

  def destroy
    @invitation.cancel!
    Notifier.invitation_resolved(@invitation)
    redirect_to settings_members_path, success: t(".cancelled")
  end

  def resend
    @invitation.resend!
    # Admin resend auto-approves; non-admin only sends if already approved
    if Current.user.admin?
      @invitation.update!(admin_approved: true) unless @invitation.admin_approved?
      InvitationMailer.invitation(@invitation).deliver_later
      Notifier.invitation_resolved(@invitation)
    elsif @invitation.admin_approved?
      InvitationMailer.invitation(@invitation).deliver_later
    end
    redirect_to settings_members_path, success: t(".resent", email: @invitation.email)
  end

  private

  def set_invitation
    @invitation = Current.workspace.invitations.find(params[:id])
  end

  def invitation_params
    params.require(:invitation).permit(:email)
  end
end
