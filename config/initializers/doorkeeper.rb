# frozen_string_literal: true

# Doorkeeper powers the public REST API's OAuth 2.0 layer. Campbooks uses the
# **client_credentials** grant only: a customer mints a client_id/secret in
# Settings → API access, exchanges them at POST /api/oauth/token for a bearer
# token, and calls /api/v1/*. There is no resource owner — the token is bridged
# to a workspace + acting user via columns on oauth_applications (see
# config/initializers/doorkeeper_application_extensions.rb and
# Api::V1::BaseController#establish_acting_identity!).
#
# The scope symbols below are mirrored (with human descriptions) in Api::Scopes
# (app/models/api/scopes.rb) for the Settings UI; a spec asserts the two stay in
# sync. We can't reference Api::Scopes here — autoloading an app constant during
# boot is unsafe — so the canonical list is these literals.
Doorkeeper.configure do
  orm :active_record

  # client_credentials never authenticates a resource owner, so this block is
  # never invoked. It is required to be present; raise loudly if some future
  # mis-config (e.g. enabling authorization_code) ever reaches it.
  resource_owner_authenticator do
    raise "resource_owner_authenticator must not be called: Campbooks only uses " \
          "the client_credentials grant (no resource owner)."
  end

  # Lock the provider to client_credentials. Disables authorization_code,
  # implicit, and password flows entirely.
  grant_flows %w[client_credentials]

  # Short-lived bearer tokens. Clients request a fresh one when they get a 401.
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
                  :"documents:read", :"documents:write",
                  :"contacts:read", :"contacts:write",
                  :"tags:read", :"tags:write",
                  :"document_types:read",
                  :"workflows:read", :"workflows:trigger",
                  :"scout:read", :"scout:write"
  enforce_configured_scopes

  # Raise on auth failures so Api::V1::BaseController's rescue_from renders our
  # JSON error envelope ({ error: { code, message } }) consistently.
  handle_auth_errors :raise

  # Keep the surface to exactly the token + revoke endpoints. Token introspection
  # (RFC 7662) is not part of the Campbooks API contract; clients simply re-fetch
  # a token on a 401.
  allow_token_introspection false
end
