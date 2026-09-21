# frozen_string_literal: true

# Time (merged agenda, focus blocks, asks, reminders) + Money (evidence, reconciliation, loans). See api-migration/time-money.md.
# Owned by the api_app_time_money build agent — add routes only under the namespaces below;
# do NOT edit config/routes.rb or other api_app_*.rb files. Session-bearer auth
# via Api::App::BaseController. Controllers live in app/controllers/api/app/.
namespace :api do
  namespace :app do
    # ── TIME ──────────────────────────────────────────────────────────────────
    # GET /api/app/time?view=agenda|week|month&date=YYYY-MM-DD
    get "time", to: "time/agenda#show"

    # One-shot time-zone capture (SPA fires once when user.time_zone is blank).
    patch "account/time_zone", to: "time/account#time_zone"

    # Ask (Task) mutations from the Time surface.
    scope "asks/:id" do
      post  "hold",       to: "time/asks#hold"
      patch "schedule",   to: "time/asks#schedule"
      post  "snooze",     to: "time/asks#snooze"
      post  "done",       to: "time/asks#done"
      post  "dismiss",    to: "time/asks#dismiss"
      post  "hand_off",   to: "time/asks#hand_off"
      post  "take_back",  to: "time/asks#take_back"
    end

    # FocusBlock mutations.
    scope "focus_blocks/:id" do
      post   "keep",    to: "time/focus_blocks#keep"
      patch  "move",    to: "time/focus_blocks#move"
      delete "",        to: "time/focus_blocks#dismiss", as: :api_app_focus_block
    end

    # Reminder mutations from the Time surface.
    scope "reminders/:id" do
      post   "confirm",  to: "time/reminders#confirm"
      delete "",         to: "time/reminders#dismiss", as: :api_app_reminder
      post   "snooze",   to: "time/reminders#snooze"
    end

    # ── MONEY ─────────────────────────────────────────────────────────────────
    # Main money read model.
    get  "money",                              to: "money/money#index"
    get  "money/export",                       to: "money/money#export"
    post "money/reconcile_statements",         to: "money/money#reconcile_statements"

    # Obligation mutations.
    scope "money/obligations/:id" do
      post   "chase",          to: "money/money#chase"
      patch  "settle",         to: "money/money#settle"
      patch  "unsettle",       to: "money/money#unsettle"
      post   "confirm_line",   to: "money/money#confirm_line"
      post   "set_aside_line", to: "money/money#set_aside_line"
      post   "reset_line",     to: "money/money#reset_line"
    end

    # Loans.
    get  "money/loans",           to: "money/loans#index"
    post "money/loans",           to: "money/loans#create"
    get  "money/loans/:id",       to: "money/loans#show",   as: :api_app_money_loan
    patch "money/loans/:id",      to: "money/loans#update"
    delete "money/loans/:id",     to: "money/loans#destroy"
    post "money/loans/:id/dismiss", to: "money/loans#dismiss"

    # Reconciliations (bank statements).
    resources :reconciliations, only: %i[index create show destroy],
              controller: "money/reconciliations" do
      member do
        post :confirm_all_suggestions
        get  :export
        post :retry_parse
        get  :download
      end

      # Bank transaction workbench actions, nested under reconciliations.
      resources :bank_transactions, only: [],
                controller: "money/bank_transactions" do
        member do
          post :confirm
          post :reject
          post :exclude
          post :reset
          post :manual_match
          post :request_invoice
          post :upload_and_link
          get  :resolve_panel
        end
      end
    end
  end
end
