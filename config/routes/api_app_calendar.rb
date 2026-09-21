# frozen_string_literal: true

# Calendar views, event CRUD/RSVP/reschedule, event types, calendar account + per-calendar management, visibilities, ICS import. See api-migration/calendar.md.
# Owned by the api_app_calendar build agent — add routes only under the namespaces below;
# do NOT edit config/routes.rb or other api_app_*.rb files. Session-bearer auth
# via Api::App::BaseController. Controllers live in app/controllers/api/app/.
namespace :api do
  namespace :app do
    # GET /api/app/calendar?view=agenda|day|week|month&date=YYYY-MM-DD
    # Screen payload: sidebar accounts + event collections for the requested view.
    get "calendar", to: "calendar/calendar#index"

    # Calendar events: show, create, update, async delete, RSVP, reschedule.
    resources :calendar_events, only: %i[show create update destroy], module: "calendar" do
      member do
        post  :rsvp
        patch :reschedule
      end
    end

    # Event types: workspace-scoped CRUD + one-click starter set.
    resources :event_types, only: %i[index create update destroy], module: "calendar" do
      collection do
        post :starters
      end
    end

    # Calendar accounts: rename, disconnect, sharing panel, on-demand refresh,
    # and nested per-calendar sync/color toggle.
    resources :calendar_accounts, only: %i[update destroy], module: "calendar" do
      member do
        get :sharing
      end
      collection do
        post :refresh
      end
      # Nested per-calendar settings — no extra module: the parent already sets
      # the api/app/calendar namespace.
      resources :calendars, only: %i[update]
    end

    # Per-user show/hide toggle for a single calendar.
    resources :calendar_visibilities, only: %i[update], module: "calendar"

    # ICS file import into a writable calendar.
    # controller: explicit to avoid singlar-resource plural-controller mismatch.
    post "calendar_import", to: "calendar/calendar_import#create"
  end
end
