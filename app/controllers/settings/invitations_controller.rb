class Settings::InvitationsController < Settings::BaseController
  before_action :set_invitation, only: [ :approve, :destroy, :resend ]
  before_action :require_invitation_manager, only: [ :destroy, :resend ]

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

  # A workspace admin releases a teammate's invitation that is awaiting
  # approval (cloud mode). Instance operators keep their own global queue at
  # /admin/invitations for moderation.
  def approve
    unless Current.user.admin?
      redirect_to settings_members_path, error: t("settings.invitations.not_allowed")
      return
    end

    @invitation.approve_by_admin!
    Notifier.invitation_resolved(@invitation)
    redirect_to settings_members_path, success: t(".approved", email: @invitation.email)
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

  # Cancelling or resending an invitation is for the person who sent it or a
  # workspace admin — not any passing member.
  def require_invitation_manager
    return if Current.user.admin? || @invitation.invited_by_id == Current.user.id

    redirect_to settings_members_path, error: t("settings.invitations.not_allowed")
  end

  def invitation_params
    params.require(:invitation).permit(:email)
  end
end
