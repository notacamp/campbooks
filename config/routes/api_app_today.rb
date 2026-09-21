# frozen_string_literal: true

# Today — the assistant-first cross-surface "needs you" worklist (locked core-UX
# model: Today · Inbox · Books · Calendar + Scout spine). A thin READ-ONLY
# aggregator; item actions route back to the existing People/Money/Time endpoints.
# Owned by the api_app_today build agent — add routes only under the namespaces
# below. Session-bearer auth via Api::App::BaseController.
namespace :api do
  namespace :app do
    # GET /api/app/today — the assistant-first Today surface: a ranked, finite
    # "needs you" worklist aggregated from People, Money (gated), and Time.
    get "today", to: "today#show"
  end
end
