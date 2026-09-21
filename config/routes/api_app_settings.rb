# frozen_string_literal: true

# Settings (~30 controllers), integrations, notifications center.
# See api-migration/settings.md.
# Owned by the api_app_settings build agent — add routes only under the namespaces below;
# do NOT edit config/routes.rb or other api_app_*.rb files. Session-bearer auth
# via Api::App::BaseController. Controllers live in app/controllers/api/app/.
namespace :api do
  namespace :app do
    # ── Account ──────────────────────────────────────────────────────────────
    # Controller: Api::App::Settings::AccountController
    resource :account, only: %i[show update destroy],
                       controller: "settings/account" do
      patch :language
      patch :compose_preference
      patch :writing_style
      post  :analyze_writing_style
      post  :export
      get   :export, action: :download_export
    end

    # ── Settings namespace ────────────────────────────────────────────────────
    # All controllers here are Api::App::Settings::* (path: settings/*)
    namespace :settings, module: "settings" do
      resource :workspace, only: %i[show update], controller: "workspace"
      resource :plan, only: :show, controller: "plan"

      resources :members, only: %i[index update]

      resources :invitations, only: %i[create destroy] do
        member do
          post :resend
          post :approve
        end
      end

      # ── AI settings ──────────────────────────────────────────────────────
      resource :ai, only: :show, controller: "ai" do
        post :switch_mode
        patch :embeddings
      end

      resources :ai_adapters, only: %i[index create update destroy]

      resources :ai_prompts, only: %i[index show update], param: :purpose

      resource :data_privacy, only: %i[show update], controller: "data_privacy"

      # ── Integrations ──────────────────────────────────────────────────────
      namespace :integrations do
        get "/", to: "index#show", as: :overview
        resource :notion, only: %i[show update], controller: "notion" do
          delete "workspaces/:id", action: :destroy, as: :workspace
        end
        resource :calendars, only: :show
        resources :connections, only: %i[index show create update destroy]
      end

      # ── Scout memory ────────────────────────────────────────────────────
      resource :memory, only: :show, controller: "memory" do
        post :teach
        resources :entries, only: %i[destroy] do
          member { post :confirm }
        end
      end
    end

    # ── Inbox settings ────────────────────────────────────────────────────────
    # Controllers: Api::App::Settings::InboxSettings::*
    namespace :inbox_settings, module: "settings/inbox_settings" do
      resources :tags, only: %i[index show create update destroy] do
        member do
          patch :toggle_hidden
          post  :merge, action: :commit_merge
        end
      end

      resources :rules, only: %i[index show create update destroy] do
        collection { get :match_count }
        member do
          patch :toggle
          post  :run
        end
        resources :runs, only: [] do
          member { post :undo, controller: "rules" }
        end
      end

      resources :signatures, only: %i[index show create update destroy] do
        member { post :set_default }
      end

      resources :document_types, only: %i[index show create update destroy]

      resources :tag_groups, only: %i[index create update destroy]

      resource :filtering, only: %i[show update], controller: "filtering" do
        post :set_sender
      end
    end

    # ── Notification center ───────────────────────────────────────────────────
    # Controller: Api::App::NotificationsController
    resources :notifications, only: %i[index show destroy] do
      collection do
        post :mark_all_read
        post :archive_all
      end
      member do
        post :mark_read
        post :archive
        post :unarchive
      end
    end

    # ── Notification preferences ─────────────────────────────────────────────
    # Controller: Api::App::NotificationPreferencesController
    scope controller: "notification_preferences" do
      get   "notification_preferences",             action: :index
      patch "notification_preferences/toggle",      action: :toggle
      patch "notification_preferences/bulk_toggle", action: :bulk_toggle
    end

    # Digest preference hangs off notification_preferences controller too
    patch "settings/notifications/digest_preference",
          to: "notification_preferences#digest_preference"
  end
end
