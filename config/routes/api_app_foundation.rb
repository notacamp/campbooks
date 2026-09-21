# frozen_string_literal: true

# Foundation routes for the first-party app API (/api/app): the identity
# bootstrap plus the session-bearer lifecycle (password login, sign out, and the
# native one-time-token exchange). Owned by the migration foundation — see
# api-migration/00-auth.md. Every other surface has its own api_app_*.rb file.
namespace :api do
  namespace :app do
    # GET /api/app/me — the bootstrap payload the SPA loads on launch.
    get "me", to: "me#show"

    # Session bearer lifecycle. `create` = password login (may 401 with
    # `mfa_required` + an mfa_token); the challenge submit and the whole
    # registration/2FA/onboarding surface live in api_app_auth.rb.
    resource :session, only: %i[create destroy], controller: "sessions"

    # POST /api/app/oauth/native/exchange — swap the one-time :native_session
    # token minted by OauthNativeHandoff for a real session bearer (Capacitor /
    # SPA deep-link handoff).
    post "oauth/native/exchange", to: "sessions#native_exchange"
  end
end
