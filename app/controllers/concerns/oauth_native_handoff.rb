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

    # An authenticated account-link flow from the React SPA: the identity rides in
    # the HMAC-signed state (no session cookie crosses into the OAuth auth window).
    # Only fires on a valid, verified state with spa: true.
    def spa_account_link_flow?
      oauth_state["verified"] && oauth_state["spa"] &&
        oauth_state["flow"] == "account_link"
    end

    # A sign-in flow from the React SPA. No user exists yet, so (unlike the
    # account-link flows) identity comes from the OAuth resolution, not the state.
    # Only fires on a verified (HMAC-valid, unexpired) state with spa: true.
    def spa_sign_in_flow?
      oauth_state["verified"] && oauth_state["spa"] &&
        oauth_state["flow"] == "sign_in"
    end

    # Whether an unmatched identity may self-serve-create a workspace on this
    # sign-in. The SPA respects signup_mode (blocks on beta_code cloud); web +
    # native keep their existing create-on-first-sign-in behaviour.
    def sign_in_allow_create?
      return true unless spa_sign_in_flow?

      public_signup_allowed?
    end

    # Flows allowed to run without a session cookie: sign-in (no user yet), a
    # native authenticated flow, or a SPA account-link flow.
    def unauthenticated_oauth_flow?
      sign_in_flow? || native_authenticated_flow? || spa_account_link_flow?
    end

    def native_oauth?
      oauth_state["native"]
    end

    # For a native or SPA account-link, authenticate from the verified state so
    # the existing handlers (which read Current.user / Current.workspace) work
    # unchanged. Workspace is derived from the user — never trusted from the wire.
    def assume_native_identity
      return unless native_authenticated_flow? || spa_account_link_flow?

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
      if spa_sign_in_flow?
        # SPA sign-in: hand a one-time :native_session token to the SPA return_to;
        # the client swaps it for a bearer via POST /api/app/oauth/native/exchange
        # (no bearer ever on the URL). Provider-MFA only — the same documented
        # exception as native, since the one-time-token handoff carries no app
        # second factor. See mfa_oauth_bypass_spec.
        redirect_to spa_callback_url(token: user.generate_token_for(:native_session)),
                    allow_other_host: true
      elsif native_oauth?
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
      elsif spa_sign_in_flow?
        # SPA sign-in blocked (unknown email on a gated cloud, email already owns
        # an account, etc.) → bounce to the SPA with the reason; no session minted.
        redirect_to spa_callback_url(error: result.reason), allow_other_host: true
      else
        message = t("auth.oauth_sign_in.blocked.#{result.reason}", provider: oauth_provider.to_s.titleize)
        redirect_to new_session_path, flash: { result.severity => message }
      end
    end

    # Finish an account-link: native pops back into the app; SPA redirects to the
    # return_to URL embedded in the signed state; web redirects as before.
    def complete_oauth_account_link(success_message)
      if native_oauth?
        redirect_to_native(flow: "connect", status: "success")
      elsif spa_account_link_flow?
        redirect_to spa_callback_url(status: "success"), allow_other_host: true
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

    # ── SPA OAuth helpers ────────────────────────────────────────────────────

    # Root URL of the React SPA frontend. Reads APP_FRONTEND_URL (set in
    # production); falls back to localhost:3100 for local development.
    def spa_frontend_url(path = "/")
      root = ENV.fetch("APP_FRONTEND_URL", "http://localhost:3100").chomp("/")
      path.start_with?("/") ? "#{root}#{path}" : "#{root}/#{path}"
    end

    # Build the SPA redirect URL after a successful or failed OAuth account-link.
    # `return_to` from the signed state is trusted (signature already verified)
    # but we still validate the origin to prevent an open-redirect if the state
    # key is misused via a cross-site leak (belt-and-suspenders).
    def spa_callback_url(**extra_params)
      raw = oauth_state["return_to"].to_s.strip
      frontend_root = spa_frontend_url("/")

      base = if raw.present? && (raw.start_with?(frontend_root) || raw.start_with?("/"))
        raw.start_with?("/") ? spa_frontend_url(raw) : raw
      else
        spa_frontend_url("/settings/mailboxes")
      end

      extra_params.any? ? "#{base}#{base.include?("?") ? "&" : "?"}#{extra_params.to_query}" : base
    end

    # Convenience: SPA redirect URL for callback errors.
    def spa_callback_error_url(reason)
      spa_callback_url(error: reason)
    end
end
