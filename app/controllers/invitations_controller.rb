class InvitationsController < ApplicationController
  allow_unauthenticated_access
  before_action :set_invitation
  before_action :check_validity

  def show
    if authenticated?
      if Current.user.workspace == @invitation.workspace
        redirect_to root_path, success: t(".already_member")
        return
      end
      render :show
    else
      session[:invitation_token] = @invitation.token
      redirect_to new_registration_path, success: t(".create_account")
    end
  end

  def accept
    unless authenticated?
      session[:invitation_token] = @invitation.token
      redirect_to new_registration_path, success: t(".create_account")
      return
    end

    if Current.user.workspace == @invitation.workspace
      redirect_to root_path, success: t(".already_member")
      return
    end

    @invitation.accept!(Current.user)
    redirect_to root_path, success: t(".joined", workspace: @invitation.workspace.name)
  end

  private

  def set_invitation
    @invitation = Invitation.find_by!(token: params[:token])
  rescue ActiveRecord::RecordNotFound
    redirect_to root_path, error: t("invitations.set_invitation.not_found")
  end

  def check_validity
    if @invitation.accepted?
      redirect_to root_path, error: t("invitations.check_validity.already_accepted")
    elsif @invitation.cancelled?
      redirect_to root_path, error: t("invitations.check_validity.cancelled")
    elsif @invitation.expired?
      redirect_to root_path, error: t("invitations.check_validity.expired")
    elsif !Rails.application.config.self_hosted && !@invitation.admin_approved?
      redirect_to root_path, error: t("invitations.check_validity.pending_approval")
    end
  end
end
