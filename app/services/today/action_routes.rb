# frozen_string_literal: true

module Today
  # Maps (source, kind, ids) to the real /api/app endpoint that performs the
  # action. Each method returns a hash subset of:
  #   { endpoint:, method:, body:, undo: }
  # that is merged onto the base action hash { kind:, label:, primary: }.
  #
  # When an action is navigation-only or a multi-step flow (no single mutation)
  # the method returns {} so the client handles it by kind alone.
  #
  # VERIFIED: every path is matched against config/routes/api_app_*.rb.
  # Do NOT add paths that are not declared in those route files.
  #
  # paid → POST /api/app/people/:ref_id/action { kind: "paid" } — settles the
  # person's late invoice via Api::App::People::ActionsController#do_paid. No undo
  # (settling is one-way; a landed bank match still wins later).
  #
  # NOTE — money review: `review` is the Today action kind for bank-transaction
  # workbench items, but there is no POST .../bank_transactions/:id/review route.
  # `review` is returned as navigation-only ({}) so the client navigates to the
  # Money/reconciliation surface.
  module ActionRoutes
    PEOPLE_BASE         = "/api/app/people"
    ASKS_BASE           = "/api/app/asks"
    REMINDERS_BASE      = "/api/app/reminders"
    MONEY_BASE          = "/api/app/money"
    RECONCILIATIONS_BASE = "/api/app/reconciliations"

    # People row action — all mutations go to POST /api/app/people/:ref_id/action.
    # reply is navigation-only (no endpoint).
    def self.people(kind, ref_id)
      base = "#{PEOPLE_BASE}/#{ref_id}/action"

      case kind.to_s
      when "archive"
        { endpoint: base, method: "POST", body: { "kind" => "archive" },
          undo: { endpoint: base, method: "POST", body: { "kind" => "unarchive" } } }
      when "done"
        { endpoint: base, method: "POST", body: { "kind" => "done" },
          undo: { endpoint: base, method: "POST", body: { "kind" => "undo_done" } } }
      when "star"
        { endpoint: base, method: "POST", body: { "kind" => "star" },
          undo: { endpoint: base, method: "POST", body: { "kind" => "unstar" } } }
      when "snooze"
        { endpoint: base, method: "POST", body: { "kind" => "snooze" },
          undo: { endpoint: base, method: "POST", body: { "kind" => "unsnooze" } } }
      when "paid"
        # One-way (no undo) — settles the person's late invoice.
        { endpoint: base, method: "POST", body: { "kind" => "paid" } }
      else
        # reply and unknown kinds: navigation-only — client routes by kind.
        {}
      end
    end

    # Ask (Task) mutation from the Time surface.
    # Routes verified in config/routes/api_app_time_money.rb.
    def self.ask(kind, ask_id)
      case kind.to_s
      when "done"
        { endpoint: "#{ASKS_BASE}/#{ask_id}/done", method: "POST" }
      when "snooze"
        { endpoint: "#{ASKS_BASE}/#{ask_id}/snooze", method: "POST" }
      when "schedule"
        { endpoint: "#{ASKS_BASE}/#{ask_id}/schedule", method: "PATCH" }
      when "hold"
        { endpoint: "#{ASKS_BASE}/#{ask_id}/hold", method: "POST" }
      when "dismiss"
        { endpoint: "#{ASKS_BASE}/#{ask_id}/dismiss", method: "POST" }
      else
        {}
      end
    end

    # Reminder mutation from the Time surface.
    # Routes verified in config/routes/api_app_time_money.rb.
    def self.reminder(kind, reminder_id)
      case kind.to_s
      when "confirm"
        { endpoint: "#{REMINDERS_BASE}/#{reminder_id}/confirm", method: "POST" }
      when "dismiss"
        # DELETE /api/app/reminders/:id (no suffix — the scope root with DELETE)
        { endpoint: "#{REMINDERS_BASE}/#{reminder_id}", method: "DELETE" }
      when "snooze"
        { endpoint: "#{REMINDERS_BASE}/#{reminder_id}/snooze", method: "POST" }
      else
        {}
      end
    end

    # Money surface action.
    # - reconcile → POST /api/app/money/reconcile_statements
    # - confirm/reject/exclude/reset/manual_match on a bank transaction →
    #     POST /api/app/reconciliations/:recon_id/bank_transactions/:tx_id/<kind>
    # - review: no route exists for .../bank_transactions/:id/review — navigation only.
    # - view / add_statement: navigation/flow — omit endpoint.
    def self.money(kind, tx_id: nil, recon_id: nil)
      case kind.to_s
      when "reconcile"
        { endpoint: "#{MONEY_BASE}/reconcile_statements", method: "POST" }
      when "confirm", "reject", "exclude", "reset", "manual_match"
        return {} unless tx_id && recon_id

        {
          endpoint: "#{RECONCILIATIONS_BASE}/#{recon_id}/bank_transactions/#{tx_id}/#{kind}",
          method: "POST"
        }
      else
        # view, add_statement, review, and unknown kinds: navigation-only.
        {}
      end
    end
  end
end
