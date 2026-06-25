# Manage OAuth sign-in methods (Settings → Security). `create` kicks off provider
# consent for the add_sign_in flow; the provider callback (Oauth::*Controller#
# handle_add_sign_in) does the actual linking via Auth::IdentityLinker. `destroy`
# unlinks an Identity, guarded so a user can never remove their only way in.
class Settings::Security::SignInMethodsController < Settings::BaseController
  include MfaEnrollment

  PROVIDERS = %w[ google microsoft zoho ].freeze

  def create
    provider = params[:provider].to_s
    # Microsoft is treated as not-offerable while gated off (Features.microsoft?),
    # so a stale "Add Microsoft" POST falls through to the unknown-provider path.
    allowed = microsoft_enabled? ? PROVIDERS : PROVIDERS - %w[ microsoft ]
    redirect_to(settings_security_path, error: t(".unknown_provider")) and return unless allowed.include?(provider)

    if current_user.identities.exists?(provider: provider)
      redirect_to(settings_security_path, notice: t(".already_linked", provider: provider.titleize)) and return
    end

    redirect_to add_sign_in_authorize_url(provider), allow_other_host: true
  end

  def destroy
    identity = current_user.identities.find(params[:id])

    # Never strip the last way in. The identity being removed is itself counted,
    # so a count of 1 means it's the only method.
    redirect_to(settings_security_path, error: t(".last_method")) and return if current_user.sign_in_methods_count <= 1
    return unless reauthenticated_for_sign_in_method_change?

    identity.destroy
    AuditEvent.log("sign_in_method_removed", user: current_user, request: request, provider: identity.provider)
    redirect_to settings_security_path, success: t(".removed", provider: identity.provider_label)
  end

  private

  # Password users re-confirm with their password (mirrors passkey/TOTP removal —
  # a stolen session shouldn't be able to strip logins). OAuth-only users have no
  # password to confirm; the last-method guard above is their protection.
  def reauthenticated_for_sign_in_method_change?
    return true unless current_user.password_set_by_user?

    reauthenticated_for_security_change?
  end

  # Provider consent URL for the add_sign_in flow. Mirrors
  # SessionsController#provider_authorize_url but carries flow=add_sign_in and the
  # user id (the callback runs without our cookie in the native shell). Sign-in
  # scopes only — we want identity, not mailbox access.
  def add_sign_in_authorize_url(provider)
    state = Oauth::State.encode(flow: "add_sign_in", native: hotwire_native_app?, user_id: current_user.id)
    case provider
    when "google"
      Google::OauthClient.authorize_url(redirect_uri: oauth_gmail_callback_url, state: state)
    when "microsoft"
      Microsoft::OauthClient.authorize_url(redirect_uri: oauth_microsoft_callback_url, state: state)
    when "zoho"
      zoho_params = {
        client_id: ENV.fetch("ZOHO_CLIENT_ID"),
        response_type: "code",
        redirect_uri: oauth_zoho_callback_url,
        scope: "ZohoMail.accounts.READ",
        access_type: "offline",
        prompt: "consent",
        state: state
      }
      "#{Zoho::OauthClient::AUTH_URL}?#{zoho_params.to_query}"
    end
  end
end
