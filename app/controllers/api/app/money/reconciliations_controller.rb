# frozen_string_literal: true

module Api
  module App
    module Money
      # Bank-statement reconciliation endpoints. Mirrors ReconciliationsController +
      # Reconciliations::BankTransactionsController but returns JSON.
      #
      # GET    /api/app/reconciliations
      # POST   /api/app/reconciliations         (multipart: statement_file or statement_document_id)
      # GET    /api/app/reconciliations/:id
      # DELETE /api/app/reconciliations/:id
      # POST   /api/app/reconciliations/:id/confirm_all_suggestions
      # GET    /api/app/reconciliations/:id/export    (CSV download)
      # POST   /api/app/reconciliations/:id/retry_parse
      # GET    /api/app/reconciliations/:id/download  (original file redirect)
      #
      # Bank-transaction actions (under /reconciliations/:rec_id/bank_transactions/:id/...):
      # POST confirm | reject | exclude | reset | manual_match | request_invoice | upload_and_link
      # GET  resolve_panel
      class ReconciliationsController < Api::App::BaseController
        before_action :require_accounting_enabled
        before_action :require_accounting_entitlement
        before_action :set_reconciliation, only: %i[
          show destroy confirm_all_suggestions export retry_parse download
        ]

        VALID_EXCLUSION_REASONS = Reconciliations::BankTransactionsController::VALID_EXCLUSION_REASONS

        # GET /api/app/reconciliations
        def index
          @pagy, reconciliations = pagy(
            current_workspace.reconciliations.recent.includes(:statement_document),
            limit: per_page
          )

          rids     = reconciliations.map(&:id)
          totals   = BankTransaction.where(reconciliation_id: rids).group(:reconciliation_id).count
          resolved = BankTransaction.where(reconciliation_id: rids, status: BankTransaction::RESOLVED_STATUSES)
                                    .group(:reconciliation_id).count

          data = reconciliations.map do |r|
            ReconciliationSerializer.new(r,
                                         resolved: resolved.fetch(r.id, 0),
                                         total:    totals.fetch(r.id, 0)).as_json
          end

          render_page(data, @pagy)
        end

        # POST /api/app/reconciliations
        def create
          doc = resolve_or_create_statement_document
          return unless doc

          existing = doc.reconciliations_as_statement.where.not(status: :failed).order(:created_at).first
          if existing
            return render_data(ReconciliationSerializer.new(existing).as_json, status: :ok)
          end

          reconciliation = current_workspace.reconciliations.new(
            created_by:         current_user,
            statement_document: doc,
            bank_name:          doc.metadata&.dig("bank_name"),
            currency:           doc.currency.presence || "EUR"
          )

          if reconciliation.save
            Reconciliations::ParseJob.perform_later(reconciliation.id)
            render_data(ReconciliationSerializer.new(reconciliation).as_json, status: :created)
          else
            render_error("invalid", reconciliation.errors.full_messages.to_sentence,
                         status: :unprocessable_entity)
          end
        end

        # GET /api/app/reconciliations/:id
        def show
          @pagy, transactions = pagy(
            @reconciliation.bank_transactions.ordered
                           .includes(transaction_matches: :document),
            limit: per_page
          )
          data = {
            reconciliation: ReconciliationSerializer.new(@reconciliation).as_json,
            transactions:   transactions.map { |t| BankTransactionSerializer.new(t).as_json }
          }
          render_page(data, @pagy)
        end

        # DELETE /api/app/reconciliations/:id
        def destroy
          @reconciliation.destroy
          head :no_content
        end

        # POST /api/app/reconciliations/:id/confirm_all_suggestions
        def confirm_all_suggestions
          suggested_txns = @reconciliation.bank_transactions
                                          .where(status: :suggested)
                                          .includes(transaction_matches: :document)
          confirmed_count = 0

          ActiveRecord::Base.transaction do
            suggested_txns.each do |txn|
              best_match = txn.transaction_matches.select(&:suggested?).max_by(&:confidence)
              next unless best_match
              next unless best_match.reload.suggested?

              best_match.update!(status: :confirmed)
              txn.update!(status: :matched)
              confirmed_count += 1
            end
          end

          render_data({ confirmed_count: confirmed_count, money: refreshed_money_page })
        end

        # GET /api/app/reconciliations/:id/export
        def export
          Reconciliations::ExportJob.perform_later(@reconciliation.id)
          render_data({ status: "queued" })
        end

        # POST /api/app/reconciliations/:id/retry_parse
        def retry_parse
          unless @reconciliation.failed?
            return render_error("not_failed", "Reconciliation is not in a failed state.", status: :unprocessable_entity)
          end

          @reconciliation.update!(status: :pending, parse_error: nil)
          Reconciliations::ParseJob.perform_later(@reconciliation.id)
          render_data(ReconciliationSerializer.new(@reconciliation).as_json)
        end

        # GET /api/app/reconciliations/:id/download
        def download
          unless @reconciliation.export_generated?
            return render_error("not_ready", "Export is not ready yet.", status: :unprocessable_entity)
          end

          redirect_to rails_blob_url(@reconciliation.export_zip, disposition: "attachment"),
                      allow_other_host: true
        end

        private

        def require_accounting_enabled
          render_not_found unless Features.accounting?
        end

        def require_accounting_entitlement
          require_entitlement!(:accounting)
        end

        def set_reconciliation
          @reconciliation = current_workspace.reconciliations.find(params[:id])
        end

        def resolve_or_create_statement_document
          if params[:statement_document_id].present?
            doc = current_workspace.documents.find_by(id: params[:statement_document_id])
            return render_not_found unless doc

            doc
          elsif params[:statement_file].present?
            file = params[:statement_file]
            doc = current_workspace.documents.new(
              source:        :manual_upload,
              document_type: :bank_statement,
              ai_status:     :skipped,
              review_status: :pending
            )
            doc.original_file.attach(file)
            unless doc.save
              render_error("upload_failed", doc.errors.full_messages.to_sentence,
                           status: :unprocessable_entity)
              return nil
            end
            doc
          else
            render_error("no_source", "Provide statement_file or statement_document_id.",
                         status: :bad_request)
            nil
          end
        end

        def refreshed_money_page
          Api::App::Money::PageSerializer.new(
            ::Money::Page.for(current_workspace, current_user, today: ::Date.current)
          ).as_json
        end
      end
    end
  end
end
