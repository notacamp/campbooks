# frozen_string_literal: true

# Email message/thread reads, compose + compose-AI (chat, rewrite), drafts,
# inline uploads, email/IMAP account connect + management.
# See api-migration/compose-email.md for the full endpoint list.
# Owned by the api_app_compose_email build agent — add routes only under the
# namespaces below; do NOT edit config/routes.rb or other api_app_*.rb files.
# Session-bearer auth via Api::App::BaseController.
# Controllers live in app/controllers/api/app/{email,compose,email_accounts}/.
namespace :api do
  namespace :app do
    # ── Email messages ────────────────────────────────────────────────────────
    resources :email_messages, only: %i[index show],
                               controller: "email/messages" do
      collection do
        get  :search
        post :bulk, to: "email/bulk#create"
      end
      member do
        post   :dismiss_todo
        post   :dismiss_follow_up
        post   :follow,   to: "email/follows#create"
        delete :follow,   to: "email/follows#destroy"
        post   :tool,     to: "email/actions#create"
        resources :tags,  only: %i[create destroy],
                          controller: "email/tags",
                          param: :tag_id
      end
    end

    # ── Email threads ─────────────────────────────────────────────────────────
    resources :email_threads, only: %i[index show],
                              controller: "email/threads" do
      member do
        post   :follow,   to: "email/threads#follow"
        delete :follow,   to: "email/threads#unfollow"
      end
    end

    # ── Drafts ────────────────────────────────────────────────────────────────
    resources :drafts, controller: "email/drafts" do
      member do
        post :dismiss
        post :undismiss
      end
    end

    # ── Compose ──────────────────────────────────────────────────────────────
    # All compose routes live under /api/app/compose/ so the SPA can namespace
    # its fetch calls cleanly. Note: reply also accepts email_message_id as a
    # body param (not a URL segment) because the SPA always has the message id.
    scope :compose do
      post :send,        to: "compose/messages#send_new"
      post :reply,       to: "compose/messages#reply"
      post :rewrite,     to: "compose/messages#rewrite"
      get  :prefill,     to: "compose/messages#compose_prefill"
      post :chat,        to: "compose/chat#create"
      post :images,      to: "compose/images#create"
      post :attachments, to: "compose/attachments#create"
    end

    # ── Email accounts ────────────────────────────────────────────────────────
    resources :email_accounts, only: %i[index update destroy],
                               controller: "email_accounts/accounts" do
      member do
        get   :sharing, to: "email_accounts/accounts#sharing_show"
        patch :sharing, to: "email_accounts/accounts#sharing_update"
        get   :popover, to: "email_accounts/accounts#popover"
      end
    end

    # ── OAuth authorize URL (returns URL; does NOT redirect) ─────────────────
    # The SPA/Capacitor opens this URL in the system browser. See
    # api-migration/compose-email.md § Open questions for the connect flow.
    namespace :oauth do
      get :authorize_url, to: "email_accounts/oauth#authorize_url"
    end

    # ── IMAP accounts ─────────────────────────────────────────────────────────
    resources :imap_accounts, only: %i[create update],
                              controller: "email_accounts/imap"
  end
end
