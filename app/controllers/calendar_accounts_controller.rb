class CalendarAccountsController < ApplicationController
  before_action :require_authentication
  before_action :set_calendar_account, only: [ :update, :destroy, :sharing ]

  # Owner-only panel listing who has access, with a per-person role selector.
  def sharing
    unless @calendar_account.owned_by?(Current.user)
      redirect_to settings_integrations_calendars_path, error: t(".owner_only")
      return
    end
    load_sharing_assigns
  end

  def update
    # Renaming (managers) vs changing who has access (owner only) — same endpoint,
    # different authority, mirroring EmailAccountsController#update.
    if params[:calendar_account]
      return deny_update(t(".not_permitted")) unless @calendar_account.managed_by?(Current.user)
      update_account_settings
    else
      return deny_update(t(".owner_only_access")) unless @calendar_account.owned_by?(Current.user)
      update_user_permissions
    end
  end

  def destroy
    unless @calendar_account.owned_by?(Current.user)
      redirect_to settings_integrations_calendars_path, error: t(".owner_only")
      return
    end

    @calendar_account.deactivate!
    # GDPR: revoke at the provider too, unless a still-connected sibling (e.g. the
    # mailbox sharing this OAuth grant) is still using the token.
    Accounts::TokenRevoker.revoke_if_unshared(@calendar_account)
    redirect_to settings_integrations_calendars_path, success: t(".disconnected", name: @calendar_account.display_name)
  end

  private

  def update_account_settings
    if @calendar_account.update(calendar_account_params)
      redirect_to settings_integrations_calendars_path, success: t(".renamed", name: @calendar_account.display_name)
    else
      redirect_to settings_integrations_calendars_path,
                  error: @calendar_account.errors.full_messages.to_sentence.presence || t(".rename_failed")
    end
  end

  def update_user_permissions
    target_user = Current.workspace.users.find_by(email_address: params[:user_email]&.strip&.downcase)
    return redirect_to_sharing(error: t(".user_not_found")) unless target_user

    entry = @calendar_account.calendar_account_users.find_or_initialize_by(user: target_user)

    if params[:remove] == "true"
      return redirect_to_sharing(error: t(".owner_access_cant_be_removed")) if entry.owner?
      entry.destroy!
      redirect_to_sharing(success: t(".access_removed", name: target_user.name))
    elsif entry.owner?
      redirect_to_sharing(error: t(".owner_role_fixed"))
    elsif CalendarAccountUser::ROLES.include?(params[:role])
      entry.role = params[:role]
      entry.save!
      redirect_to_sharing(success: t(".role_updated", name: target_user.name, role: params[:role].capitalize))
    else
      redirect_to_sharing(error: t(".invalid_role"))
    end
  end

  def redirect_to_sharing(success: nil, error: nil)
    redirect_to sharing_calendar_account_path(@calendar_account), status: :see_other, success: success, error: error
  end

  def deny_update(message)
    redirect_to settings_integrations_calendars_path, error: message
  end

  def load_sharing_assigns
    @members = @calendar_account.calendar_account_users.includes(:user).to_a
                 .sort_by { |m| [ m.owner? ? 0 : 1, m.user.name.to_s.downcase ] }
    member_ids = @members.map(&:user_id)
    @addable_users = Current.workspace.users.where.not(id: member_ids).order(:name)
  end

  def calendar_account_params
    params.require(:calendar_account).permit(:name)
  end

  def set_calendar_account
    @calendar_account = Current.user.calendar_accounts.find(params[:id])
  end
end
