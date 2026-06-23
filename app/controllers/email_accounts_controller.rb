class EmailAccountsController < ApplicationController
  before_action :require_authentication
  before_action :set_email_account, only: [ :update, :destroy, :sharing ]

  def create
    provider = params[:provider] || "zoho"

    # Microsoft 365 mailbox connect is hidden/disabled until the Entra app is
    # wired up — refuse it server-side too so a stale link can't reach the broken
    # flow. (Toggle with ENABLE_MICROSOFT_MAILBOX=1.)
    if provider == "microsoft" && !microsoft_mailbox_connect_enabled?
      back = session[:onboarding_return_to] || email_messages_path(inbox_settings: "accounts")
      redirect_to(back, alert: t(".microsoft_unavailable", default: "Microsoft 365 connections aren't available yet.")) and return
    end

    if provider == "microsoft"
      state = Oauth::State.encode(flow: "account_link", native: hotwire_native_app?, user_id: (Current.user.id if hotwire_native_app?))
      redirect_to Microsoft::OauthClient.authorize_url(
        redirect_uri: oauth_microsoft_callback_url,
        state: state
      ), allow_other_host: true
    elsif provider == "google"
      state = Oauth::State.encode(flow: "account_link", native: hotwire_native_app?, user_id: (Current.user.id if hotwire_native_app?))
      redirect_to Google::OauthClient.authorize_url(
        redirect_uri: oauth_gmail_callback_url,
        state: state,
        scopes: Google::OauthClient::CONNECT_SCOPES
      ), allow_other_host: true
    else
      state = Oauth::State.encode(flow: "account_link", native: hotwire_native_app?, user_id: (Current.user.id if hotwire_native_app?))

      auth_url = "https://accounts.zoho.eu/oauth/v2/auth"
      params = {
        client_id: ENV.fetch("ZOHO_CLIENT_ID"),
        response_type: "code",
        redirect_uri: oauth_zoho_callback_url,
        scope: "ZohoMail.messages.ALL,ZohoMail.attachments.READ,ZohoMail.accounts.READ,ZohoMail.folders.READ,ZohoMail.tags.ALL,ZohoCalendar.event.ALL,ZohoCalendar.calendar.ALL",
        access_type: "offline",
        prompt: "consent",
        state: state
      }

      redirect_to "#{auth_url}?#{params.to_query}", allow_other_host: true
    end
  end

  # App-wide hover card for one of our own mailboxes. Served at
  # /email_accounts/:id/popover, fetched by the shared contact-popover controller.
  # Scoped to accounts the user can actually read; anything else 404s so the
  # endpoint never confirms an account they can't see.
  def popover
    @email_account = Current.user.readable_email_accounts.find_by(id: params[:id])
    return head :not_found if @email_account.nil?

    @message_count = @email_account.email_messages.count
    render layout: false
  end

  # Owner-only panel listing who has access, with a per-person role selector.
  def sharing
    unless @email_account.owned_by?(Current.user)
      redirect_to email_messages_path(inbox_settings: "accounts"), error: t(".owner_only")
      return
    end
    load_sharing_assigns
  end

  def update
    # The same endpoint handles two distinct edits with different authority:
    # renaming the account (managers) and changing who has access (owner only).
    if params[:email_account]
      return deny_update(t(".not_permitted")) unless @email_account.managed_by?(Current.user)
      update_account_settings
    else
      return deny_update(t(".owner_only_access")) unless @email_account.owned_by?(Current.user)
      update_user_permissions
    end
  end

  def destroy
    # Disconnecting deactivates the account for everyone it's shared with, so it
    # is owner-only — a read/send sharee must not be able to sever access.
    unless @email_account.owned_by?(Current.user)
      redirect_to email_messages_path(inbox_settings: "accounts"), error: t(".owner_only")
      return
    end

    @email_account.deactivate!
    Events.publish("email_account.disconnected", subject: @email_account, payload: { "email_address" => @email_account.email_address, "provider" => @email_account.provider })
    # GDPR: sever the grant at the provider too — but not if a still-connected
    # sibling (e.g. the calendar from the same OAuth grant) is using the token.
    Accounts::TokenRevoker.revoke_if_unshared(@email_account)

    redirect_to email_messages_path(inbox_settings: "accounts"), success: t(".disconnected", name: @email_account.display_name)
  end

  private

  def update_account_settings
    if @email_account.update(email_account_params)
      redirect_to email_messages_path(inbox_settings: "accounts"),
                  success: t(".renamed", name: @email_account.display_name)
    else
      redirect_to email_messages_path(inbox_settings: "accounts"),
                  error: @email_account.errors.full_messages.to_sentence.presence || t(".rename_failed")
    end
  end

  def update_user_permissions
    target_user = Current.workspace.users.find_by(email_address: params[:user_email]&.strip&.downcase)
    return redirect_to_sharing(error: t(".user_not_found")) unless target_user

    entry = @email_account.email_account_users.find_or_initialize_by(user: target_user)

    if params[:remove] == "true"
      return redirect_to_sharing(error: t(".owner_access_cant_be_removed")) if entry.owner?
      entry.destroy!
      redirect_to_sharing(success: t(".access_removed", name: target_user.name))
    elsif entry.owner?
      # The owner row is fixed; roles only apply to collaborators.
      redirect_to_sharing(error: t(".owner_role_fixed"))
    elsif EmailAccountUser::ROLES.include?(params[:role])
      entry.role = params[:role]
      entry.save!
      redirect_to_sharing(success: t(".role_updated", name: target_user.name, role: params[:role].capitalize))
    else
      redirect_to_sharing(error: t(".invalid_role"))
    end
  end

  # Sharing mutations return to the (owner-only) sharing panel so the owner can
  # keep editing. status: :see_other keeps the Turbo Frame redirect a clean GET.
  def redirect_to_sharing(success: nil, error: nil)
    redirect_to sharing_email_account_path(@email_account), status: :see_other, success: success, error: error
  end

  def deny_update(message)
    redirect_to email_messages_path(inbox_settings: "accounts"), error: message
  end

  def load_sharing_assigns
    @members = @email_account.email_account_users.includes(:user).to_a
                 .sort_by { |m| [ m.owner? ? 0 : 1, m.user.name.to_s.downcase ] }
    member_ids = @members.map(&:user_id)
    @addable_users = Current.workspace.users.where.not(id: member_ids).order(:name)
  end

  def email_account_params
    params.require(:email_account).permit(:name)
  end

  def set_email_account
    @email_account = Current.user.email_accounts.find(params[:id])
  end
end
