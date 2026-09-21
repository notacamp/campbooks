# frozen_string_literal: true

require "csv"

module Api
  module App
    module Money
      # The Money surface read + obligation mutations.
      #
      # GET  /api/app/money       — full Money::Page read model
      # GET  /api/app/money/export — CSV ledger
      # POST /api/app/money/reconcile_statements — trigger AutoStart for pending statements
      # POST /api/app/money/obligations/:id/chase   — draft chase email
      # PATCH /api/app/money/obligations/:id/settle   — mark paid
      # PATCH /api/app/money/obligations/:id/unsettle — revert
      # POST /api/app/money/obligations/:id/confirm_line — confirm workbench line from Money
      # POST /api/app/money/obligations/:id/set_aside_line
      # POST /api/app/money/obligations/:id/reset_line
      #
      # All mutations return the refreshed Money::Page read model (the surface=money
      # contract). Gated by Features.accounting? + :accounting entitlement.
      class MoneyController < Api::App::BaseController
        before_action :require_accounting_enabled
        before_action :require_accounting_entitlement
        before_action :set_obligation, only: %i[chase settle unsettle confirm_line set_aside_line reset_line]

        # GET /api/app/money
        def index
          page = build_page
          render_data(Api::App::Money::PageSerializer.new(page).as_json)
        end

        # GET /api/app/money/export
        def export
          ledger = ::Money::Ledger.for(current_workspace, current_user, today: ::Date.current)
          csv    = ledger_csv(ledger.obligations)
          send_data csv, filename: "money-export-#{::Date.current.iso8601}.csv", type: "text/csv"
        end

        # POST /api/app/money/reconcile_statements
        def reconcile_statements
          documents = Reconciliations::AutoStart.pending_for(current_workspace).to_a
          documents.each { |doc| Reconciliations::AutoStartJob.perform_later(doc.id, created_by_id: current_user.id) }
          render_data(money_page_data, status: :ok)
        end

        # POST /api/app/money/obligations/:id/chase
        def chase
          return render_gone unless @obligation&.missing?
          return render_gone unless @obligation.receivable?

          draft = ::Money::ReminderDraft.chase(@obligation)
          render_data({ draft: { subject: draft.subject, body: draft.body } })
        end

        # PATCH /api/app/money/obligations/:id/settle
        def settle
          return render_gone unless @obligation&.document

          source = params[:source].to_s.presence || "manual"
          @obligation.document.mark_settled!(source: source)
          render_data(money_page_data)
        end

        # PATCH /api/app/money/obligations/:id/unsettle
        def unsettle
          return render_gone unless @obligation&.document

          @obligation.document.mark_unsettled!
          render_data(money_page_data)
        end

        # POST /api/app/money/obligations/:id/confirm_line
        def confirm_line
          txn = workspace_transaction(params[:transaction_id])
          return render_not_found unless txn

          begin
            Reconciliations::LineActions.new(txn).confirm!(params[:match_id])
            render_data(money_page_data)
          rescue ActiveRecord::RecordNotFound
            render_error("match_not_found", "Match not found.", status: :not_found)
          end
        end

        # POST /api/app/money/obligations/:id/set_aside_line
        def set_aside_line
          txn    = workspace_transaction(params[:transaction_id])
          return render_not_found unless txn

          reason = params[:reason].to_s.strip
          unless Reconciliations::BankTransactionsController::VALID_EXCLUSION_REASONS.include?(reason)
            return render_error("invalid_reason", "Invalid exclusion reason.", status: :unprocessable_entity)
          end

          Reconciliations::LineActions.new(txn).exclude!(reason)
          render_data(money_page_data)
        end

        # POST /api/app/money/obligations/:id/reset_line
        def reset_line
          txn = workspace_transaction(params[:transaction_id])
          return render_not_found unless txn

          Reconciliations::LineActions.new(txn).reset!
          render_data(money_page_data)
        end

        private

        def require_accounting_enabled
          render_not_found unless Features.accounting?
        end

        def require_accounting_entitlement
          require_entitlement!(:accounting)
        end

        def build_page(statement_id: params[:statement])
          ::Money::Page.for(current_workspace, current_user,
                            today:        ::Date.current,
                            statement_id: statement_id)
        end

        def money_page_data
          Api::App::Money::PageSerializer.new(build_page).as_json
        end

        def set_obligation
          page    = build_page
          @obligation = page.ledger.find(params[:id])
        end

        def workspace_transaction(id)
          BankTransaction.joins(:reconciliation)
                         .where(reconciliations: { workspace_id: current_workspace.id })
                         .find_by(id: id)
        end

        def render_gone
          render_error("gone", "Resource no longer exists.", status: :not_found)
        end

        def ledger_csv(obligations)
          CSV.generate(headers: true) do |csv|
            csv << [ "ID", "Direction", "Counterpart", "What", "Amount", "Currency",
                     "Anchor Date", "Status", "Settled On" ]
            obligations.each do |ob|
              csv << [ ob.id, ob.direction, ob.counterpart, ob.what,
                       ob.amount_cents.to_f / 100.0, ob.currency,
                       ob.anchor_on&.iso8601, ob.status, ob.settled_on&.iso8601 ]
            end
          end
        end
      end
    end
  end
end
