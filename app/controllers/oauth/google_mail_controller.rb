class Oauth::GoogleMailController < ApplicationController
  include OauthNativeHandoff
  include EmailAccountCapGuard
  before_action :require_authentication, unless: :unauthenticated_oauth_flow?
  rate_limit to: 20, within: 3.minutes, only: :callback,
             with: -> { redirect_to new_session_path, error: t("auth.oauth_sign_in.rate_limited") }

  def callback
    code = params.require(:code)

    case oauth_state["flow"]
    when "sign_in"
      handle_sign_in(code)
    when "account_link"
      handle_account_link(code)
    when "add_sign_in"
      handle_add_sign_in(code)
    else
      redirect_to new_session_path, error: t(".invalid_request")
    end
  rescue => e
    Rails.logger.error("[Oauth::GoogleMailController] OAuth callback failed: #{e.message}")
    if native_oauth?
      redirect_to_native(flow: oauth_state["flow"], status: "error")
    else
      redirect_to oauth_failure_redirect, error: t(".auth_failed")
    end
  end

  private

  def handle_sign_in(code)
    @oauth_flow = "sign_in"

    oauth = Google::OauthClient.new
    token_data = oauth.exchange_code(code, oauth_gmail_callback_url)

    identity = Google::AccountDiscovery.new(token_data["access_token"]).discover_identity
    unless identity
      redirect_to new_session_path, error: t(".sign_in_failed")
      return
    end

    complete_oauth_sign_in(
      Auth::OauthSignIn.call(
        provider: oauth_provider,
        uid: identity[:account_id],
        email: identity[:email],
        name: identity[:name]
      )
    )
  end

  # Provider key for this controller — feeds Auth::OauthSignIn and the shared
  # block-message helper (OauthNativeHandoff#handle_oauth_block).
  def oauth_provider = :google

  # Attach this Google account as a sign-in method for the already-authenticated
  # user (Settings → Security). Linking lives in Auth::IdentityLinker, never in
  # the sign-in resolver. Current.user is the web session or the native state.
  def handle_add_sign_in(code)
    @oauth_flow = "add_sign_in"

    oauth = Google::OauthClient.new
    token_data = oauth.exchange_code(code, oauth_gmail_callback_url)

    identity = Google::AccountDiscovery.new(token_data["access_token"]).discover_identity
    unless identity && identity[:account_id].present?
      redirect_to settings_security_path, error: t(".discovery_failed")
      return
    end

    complete_oauth_add_sign_in(
      Auth::IdentityLinker.call(
        user: Current.user, provider: oauth_provider,
        uid: identity[:account_id], email: identity[:email]
      )
    )
  end

  def handle_account_link(code)
    @oauth_flow = "account_link"

    oauth = Google::OauthClient.new
    token_data = oauth.exchange_code(code, oauth_gmail_callback_url)

    identity = Google::AccountDiscovery.new(token_data["access_token"]).discover_identity
    unless identity
      redirect_to account_link_failure_path, error: t(".discovery_failed")
      return
    end

    existing = EmailAccount.find_by(email_address: identity[:email])
    # Plan cap only gates genuinely new mailboxes; a reconnect reuses its slot.
    return if existing.nil? && email_account_cap_reached?

    account = if existing
      existing.update!(refresh_token: token_data["refresh_token"], provider: :google, active: true)
      existing
    else
      EmailAccount.create!(
        email_address: identity[:email],
        provider_account_id: identity[:account_id],
        refresh_token: token_data["refresh_token"],
        provider: :google,
        workspace: Current.workspace
      )
    end

    account.email_account_users.find_or_create_by!(user: Current.user) do |entry|
      entry.owner = true
      entry.can_read = true
      entry.can_send = true
      entry.can_manage = true
    end

    # The same grant covers Google Calendar — provision its calendar too.
    Calendars::AccountProvisioner.call(
      email_address: identity[:email], provider: :google,
      refresh_token: token_data["refresh_token"], provider_account_id: identity[:account_id],
      workspace: Current.workspace, owner: Current.user
    )

    Events.publish("email_account.connected", subject: account, payload: { "email_address" => account.email_address, "provider" => account.provider })

    # Kick the first sync now rather than waiting up to a minute for the poll —
    # the user is watching the first-sync stage. Slot-lock dedupes any overlap;
    # a never-baselined account hands itself to the full resync.
    EmailScanJob.perform_later(account.id, "delta")

    complete_oauth_account_link(t(".linked", email: identity[:email]))
  end
end
