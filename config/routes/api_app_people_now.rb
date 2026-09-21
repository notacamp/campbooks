# frozen_string_literal: true

# People (standings/lanes/stand-note, streams, orgs, person threads, actions) + Now feed. See api-migration/people-now.md.
# Owned by the api_app_people_now build agent — add routes only under the namespaces below;
# do NOT edit config/routes.rb or other api_app_*.rb files. Session-bearer auth
# via Api::App::BaseController. Controllers live in app/controllers/api/app/.
namespace :api do
  namespace :app do
    # ── People directory ─────────────────────────────────────────────────────────
    get  "people",          to: "people/directory#index"
    get  "people/streams",  to: "people/streams#index"
    get  "people/streams/:name", to: "people/streams#show", as: nil

    # Org page must come before /:id so /orgs/:id does not match /:id with id="orgs".
    get  "people/orgs/:id", to: "people/orgs#show"

    # Person page + nested resources
    get  "people/:id",                        to: "people/person#show"
    post "people/:id/action",                 to: "people/actions#create"
    get  "people/:id/threads/:thread_id",     to: "people/threads#show"
    get  "people/:id/messages/:message_id",   to: "people/messages#show"

    # Details rail
    get   "people/:id/details",           to: "people/details#show"
    patch "people/:id/details",           to: "people/details#update"
    post  "people/:id/details/analyze",   to: "people/details#analyze"
    post  "people/:id/details/attention", to: "people/details#attention"
    post  "people/:id/details/merge",     to: "people/details#merge"

    # ── Now deck ─────────────────────────────────────────────────────────────────
    get  "now",              to: "now/deck#index"
    post "now/log/:id/undo", to: "now/log#undo"

    # ── Feed item actions ─────────────────────────────────────────────────────────
    post "feed/items/:id/act",     to: "feed/items#act"
    post "feed/items/:id/dismiss", to: "feed/items#dismiss"
    post "feed/items/:id/seen",    to: "feed/items#seen"
    post "feed/items/:id/undo",    to: "feed/items#undo"
    get  "feed/items/:id/preview", to: "feed/items#preview"

    # ── Activity feed ─────────────────────────────────────────────────────────────
    get "activity", to: "activity#index"
  end
end
