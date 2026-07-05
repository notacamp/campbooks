# frozen_string_literal: true

# Doorkeeper powers the public REST API's OAuth 2.0 layer. Two grants are enabled:
#
#   • client_credentials — headless/CI. A customer mints a client_id/secret in
#     Settings → API access and exchanges them at POST /api/oauth/token. There is
#     no resource owner — the token is bridged to a workspace + acting user via
#     columns on oauth_applications (config/initializers/doorkeeper_application_extensions.rb).
#   • authorization_code + PKCE — browser SSO for the Campbooks CLI. The user signs
#     in with their normal app session at GET /api/oauth/authorize and the issued
#     token carries them as its resource owner.
#
# Either way Api::V1::BaseController#establish_acting_identity! resolves the token
# to Current.workspace + Current.acting_user, and the app's normal permission gates
# apply unchanged.
#
# The scope symbols below are mirrored (with human descriptions) in Api::Scopes
# (app/models/api/scopes.rb) for the Settings UI; a spec asserts the two stay in
# sync. We can't reference Api::Scopes here — autoloading an app constant during
# boot is unsafe — so the canonical list is these literals.
Doorkeeper.configure do
  orm :active_record

  # Enable the headless grant (client_credentials) and the browser SSO grant
  # (authorization_code + PKCE). Implicit and password flows stay disabled.
  grant_flows %w[client_credentials authorization_code]

  # Resolve the signed-in app user for the browser authorize endpoint
  # (GET /api/oauth/authorize). Mirrors Authentication#find_session_by_cookie and
  # #request_authentication: resume the cookie session, or bounce to sign-in and
  # come back to the authorize URL (PKCE + query params intact) afterwards.
  # client_credentials never reaches this block — it has no resource owner.
  resource_owner_authenticator do
    session_record = Session.find_by(id: cookies.signed[:session_id])
    session_record = nil if session_record&.expired?

    if session_record
      session_record.user
    else
      session[:return_to_after_authenticating] = request.fullpath
      redirect_to("/session/new")
      nil
    end
  end

  # Public clients (the first-party CLI) MUST use PKCE — there is no client secret
  # to protect the authorization code exchange. Confidential client_credentials
  # apps don't use the authorization_code flow, so they're unaffected.
  force_pkce

  # Issue refresh tokens so the CLI refreshes silently instead of re-opening the
  # browser every 2 hours. client_credentials still gets none (per the OAuth spec) —
  # it re-mints on 401. Works with hash_token_secrets (the refresh token is hashed
  # at rest; the plaintext is only returned on the issuing/refresh response).
  use_refresh_token

  # Let the CLI's loopback redirect (http://127.0.0.1[:port]/callback, RFC 8252)
  # validate in production, where http redirect URIs are otherwise rejected. Only
  # loopback/localhost is exempt; every other redirect URI must still be https.
  force_ssl_in_redirect_uri do |uri|
    uri.host != "localhost" && !Doorkeeper::OAuth::Helpers::URIChecker.loopback_uri?(uri)
  end

  # Enabling authorization_code makes Doorkeeper require a redirect_uri on every
  # application by default. Confidential client_credentials apps legitimately have
  # none, so keep blank redirect URIs allowed for them; public authorization_code
  # apps (the CLI) still must register one.
  allow_blank_redirect_uri do |_grant_flows, application|
    application.nil? || application.confidential?
  end

  # Short-lived bearer tokens. Clients request a fresh one (or refresh) on a 401.
  access_token_expires_in 2.hours

  # Hash secrets/tokens at rest. The plaintext client secret is only retrievable
  # once, at creation, via application.plaintext_secret (shown once in the UI).
  # Access tokens are SHA256-hashed; the plaintext is available on the issuing
  # response and, in specs, via AccessToken#plaintext_token.
  hash_application_secrets using: "::Doorkeeper::SecretStoring::BCrypt"
  hash_token_secrets
  # NOTE: reuse_access_token is intentionally NOT enabled — it is mutually
  # exclusive with hash_token_secrets (the stored hash can't be matched back).

  # Scope catalog (Phase 1). No default_scopes: a token requested without an
  # explicit `scope` param gets none, and every endpoint requires a specific
  # scope, so the safe default is "denied" (Api::V1::BaseController surfaces a
  # clear hint). Keep in sync with Api::Scopes (guarded by a spec).
  optional_scopes :"emails:read", :"emails:write", :"emails:send",
                  :"email_accounts:read", :"email_accounts:write",
                  :"documents:read", :"documents:write",
                  :"contacts:read", :"contacts:write",
                  :"tags:read", :"tags:write",
                  :"document_types:read", :"document_types:write",
                  :"workflows:read", :"workflows:trigger",
                  :"scout:read", :"scout:write",
                  :"scheduled_emails:read", :"scheduled_emails:write",
                  :"calendar:read", :"calendar:write",
                  :"reminders:read", :"reminders:write",
                  :"tasks:read", :"tasks:write",
                  :"folders:read", :"folders:write",
                  :"templates:read", :"templates:write"
  enforce_configured_scopes

  # Raise on auth failures so Api::V1::BaseController's rescue_from renders our
  # JSON error envelope ({ error: { code, message } }) consistently.
  handle_auth_errors :raise

  # Keep the surface to exactly the token + revoke endpoints. Token introspection
  # (RFC 7662) is not part of the Campbooks API contract; clients simply re-fetch
  # a token on a 401.
  allow_token_introspection false
end
