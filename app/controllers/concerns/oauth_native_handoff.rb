# Shared logic for the OAuth callback controllers (gmail/microsoft/zoho mail) so
# they work both on the web and inside a Hotwire Native shell.
#
# In a native shell the OAuth dance happens in a *system* auth session
# (ASWebAuthenticationSession / Chrome Custom Tabs), which carries no app session
# cookie. So:
#   • sign-in  — there is no user yet; on success we mint a one-time token and
#     redirect into the app (campbooks://) which redeems it for a real session
#     in the web view (see SessionsController#native).
#   • account-link — the user is already signed in on the web side, but the auth
#     session has no cookie, so their identity rides along in the *signed* state
#     (Oauth::State). We set Current.acting_user from it before the existing
#     handler runs, then redirect back into the app.
#
# Include this BEFORE `before_action :require_authentication` so the identity is
# established before the auth gate is evaluated.
module OauthNativeHandoff
  extend ActiveSupport::Concern

  NATIVE_SCHEME = "campbooks".freeze

  included do
    before_action :assume_native_identity
  end

  private
    # Decoded once per request. See Oauth::State.
    def oauth_state
      @oauth_state ||= Oauth::State.decode(params[:state])
    end

    def sign_in_flow?
      oauth_state["flow"] == "sign_in"
    end

    # An authenticated flow kicked off from a native shell (link a mailbox, or add
    # an OAuth sign-in method): the identity must come from the signed state because
    # no session cookie reaches the system auth session.
    def native_authenticated_flow?
      oauth_state["verified"] && oauth_state["native"] &&
        %w[account_link add_sign_in].include?(oauth_state["flow"])
    end

    # Flows allowed to run without a session cookie: sign-in (no user yet) and a
    # native authenticated flow (identity rides in the signed state instead).
    def unauthenticated_oauth_flow?
      sign_in_flow? || native_authenticated_flow?
    end

    def native_oauth?
      oauth_state["native"]
    end

    # For a native account-link, authenticate from the verified state so the
    # existing handlers (which read Current.user / Current.workspace) are unchanged.
    # Workspace is derived from the user — never trusted from the wire.
    def assume_native_identity
      return unless native_authenticated_flow?

      Current.acting_user = User.find(oauth_state["user_id"])
      Current.workspace   = Current.acting_user.workspace
    end

    # Finish a sign-in resolution (Auth::OauthSignIn::Result). A blocked result —
    # the email already belongs to an account, or to a connected mailbox — bounces
    # back to the sign-in page with guidance (never a session). A successful one
    # continues below, where it still clears MFA exactly like password login.
    def complete_oauth_sign_in(result)
      return handle_oauth_block(result) if result.blocked?

      user = result.user
      if native_oauth?
        # Native sign-in is the documented exception: it stays provider-MFA only
        # (an in-webview challenge is a separate effort, and the handoff already
        # requires the installed app + a one-time token). See mfa_oauth_bypass_spec.
        redirect_to_native(flow: "signin", token: user.generate_token_for(:native_session))
      elsif user.mfa_enabled?
        # Web OAuth must clear the same second factor as password login, or a user
        # who enabled MFA is unprotected when signing in through the browser.
        start_mfa_challenge_for user
      else
        start_new_session_for user
        redirect_to after_authentication_url
      end
    end

    # A blocked resolution never creates a session. On web, return to sign-in with
    # guidance toward the right account / Settings → Security (see the i18n keys
    # under auth.oauth_sign_in.blocked); on native, surface a generic error since
    # the actionable guidance lives on the web sign-in page.
    def handle_oauth_block(result)
      if native_oauth?
        redirect_to_native(flow: "signin", status: "error")
      else
        message = t("auth.oauth_sign_in.blocked.#{result.reason}", provider: oauth_provider.to_s.titleize)
        redirect_to new_session_path, flash: { result.severity => message }
      end
    end

    # Finish an account-link: native pops back into the app; web redirects as before.
    def complete_oauth_account_link(success_message)
      if native_oauth?
        redirect_to_native(flow: "connect", status: "success")
      else
        return_to = session.delete(:onboarding_return_to)
        redirect_to (return_to || email_messages_path(inbox_settings: "accounts")), success: success_message
      end
    end

    # Finish "add an OAuth sign-in method" (Auth::IdentityLinker::Result). Native
    # pops back into the app; web returns to Settings → Security with a flash. The
    # provider label comes from the controller's own #oauth_provider.
    def complete_oauth_add_sign_in(result)
      AuditEvent.log("sign_in_method_added", user: Current.user, request: request, provider: oauth_provider) if result.status == :linked

      if native_oauth?
        redirect_to_native(flow: "add_sign_in", status: result.ok? ? "success" : "error")
      elsif result.ok?
        redirect_to settings_security_path,
          success: t("settings.security.sign_in_methods.linked", provider: oauth_provider.to_s.titleize)
      else
        redirect_to settings_security_path,
          error: t("settings.security.sign_in_methods.link_error.#{result.reason}", provider: oauth_provider.to_s.titleize)
      end
    end

    # Where a *failed* account-link should land (OAuth error, user cancelled, or
    # account discovery failed). During onboarding the connect was kicked off from
    # the email step (session[:onboarding_return_to] is set), so send them back
    # there. Dumping them on email_messages instead would bounce an incomplete
    # user all the way to the first onboarding step via
    # redirect_to_onboarding_if_incomplete. Peeks (doesn't delete) the key — the
    # email step re-sets it on render, and a non-onboarding link has it unset.
    def account_link_failure_path
      session[:onboarding_return_to] || email_messages_path(inbox_settings: "accounts")
    end

    # Where a failed web callback lands, keyed by the flow we were running: sign-in
    # → the login page, add-a-sign-in-method → Settings → Security, anything else
    # (account-link) → the connect return path.
    def oauth_failure_redirect
      case @oauth_flow
      when "sign_in" then new_session_path
      when "add_sign_in" then settings_security_path
      else account_link_failure_path
      end
    end

    # Redirect into the native app via its custom URL scheme; the system auth
    # session intercepts this scheme and hands the URL back to the app.
    def redirect_to_native(**params)
      redirect_to "#{NATIVE_SCHEME}://oauth?#{params.compact.to_query}", allow_other_host: true
    end
end
