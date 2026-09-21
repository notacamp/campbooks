# frozen_string_literal: true

# Auth surface — registration (3-step OTP), MFA challenge submit, password reset,
# invitation accept, onboarding, setup cards, tours dismissal.
# See api-migration/00-auth.md.
#
# Owned by the api_app_auth build agent — add routes only under the namespaces
# below; do NOT edit config/routes.rb or other api_app_*.rb files.
# Session-bearer auth via Api::App::BaseController.
# Controllers live in app/controllers/api/app/.
namespace :api do
  namespace :app do
    # ── MFA challenge ───────────────────────────────────────────────────────
    # POST   /api/app/session/challenge
    # GET    /api/app/session/challenge/options
    # POST   /api/app/session/challenge/email_code
    # These nest under /session via a singular resource so the paths match the
    # web routing shape (session → challenge). `only: []` keeps the session
    # resource here non-CRUD; the real session CRUD lives in api_app_foundation.rb.
    resource :session, only: [] do
      resource :challenge, only: %i[create], controller: "session_challenges" do
        get  :options,    path: "options"
        post :email_code, path: "email_code"
      end
    end

    # ── Registration (3-step) ────────────────────────────────────────────────
    # POST /api/app/registration            — step 1: name/email → OTP
    # POST /api/app/registration/verify     — step 2: OTP → verified token
    # POST /api/app/registration/resend_code
    # POST /api/app/registration/complete   — step 3: password → bearer
    resource :registration, only: %i[create], controller: "registrations" do
      post :verify
      post :resend_code
      post :complete
    end

    # ── Password reset ───────────────────────────────────────────────────────
    # POST /api/app/passwords
    # PUT  /api/app/passwords/:token
    resources :passwords, only: %i[create update], param: :token

    # ── Invitation accept ────────────────────────────────────────────────────
    # POST /api/app/invitations/:token/accept
    resources :invitations, only: [], param: :token do
      member do
        post :accept
      end
    end

    # ── Onboarding ───────────────────────────────────────────────────────────
    # GET    /api/app/onboarding
    # PATCH  /api/app/onboarding
    # POST   /api/app/onboarding/snooze
    # GET    /api/app/onboarding/first_sync_status
    # POST   /api/app/onboarding/apply_persona
    # POST   /api/app/onboarding/skip_first_sync
    # POST   /api/app/onboarding/suggest_document_types
    # POST   /api/app/onboarding/suggest_tags
    resource :onboarding, only: %i[show update], controller: "onboarding" do
      post :snooze
      get  :first_sync_status
      post :apply_persona
      post :skip_first_sync
      post :suggest_document_types
      post :suggest_tags
    end

    # ── Setup cards ──────────────────────────────────────────────────────────
    # GET   /api/app/setup/:id
    # PATCH /api/app/setup/:id
    # POST  /api/app/setup/dismiss
    resources :setup, only: %i[show update], controller: "setups" do
      collection do
        post :dismiss
      end
    end

    # ── Tours ────────────────────────────────────────────────────────────────
    # POST /api/app/tours/:key/dismiss
    resources :tours, only: [], param: :key do
      member do
        post :dismiss
      end
    end
  end
end
