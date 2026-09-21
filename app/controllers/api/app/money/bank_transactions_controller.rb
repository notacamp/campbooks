# frozen_string_literal: true

module Api
  module App
    module Money
      # Workbench actions on individual BankTransactions, nested under a
      # Reconciliation. All mutations return the updated transaction plus the
      # refreshed Money::Page (the `surface=money` contract).
      #
      # POST  .../bank_transactions/:id/confirm
      # POST  .../bank_transactions/:id/reject
      # POST  .../bank_transactions/:id/exclude
      # POST  .../bank_transactions/:id/reset
      # POST  .../bank_transactions/:id/manual_match
      # POST  .../bank_transactions/:id/request_invoice
      # POST  .../bank_transactions/:id/upload_and_link
      # GET   .../bank_transactions/:id/resolve_panel
      class BankTransactionsController < Api::App::BaseController
        before_action :require_accounting_enabled
        before_action :require_accounting_entitlement
        before_action :set_reconciliation
        before_action :set_transaction

        VALID_EXCLUSION_REASONS = Reconciliations::BankTransactionsController::VALID_EXCLUSION_REASONS

        # POST .../bank_transactions/:id/confirm
        def confirm
          Reconciliations::LineActions.new(@transaction).confirm!(params[:match_id])
          render_data(transaction_response)
        rescue ActiveRecord::RecordNotFound
          render_error("match_not_found", "Match not found.", status: :not_found)
        end

        # POST .../bank_transactions/:id/reject
        def reject
          Reconciliations::LineActions.new(@transaction).reject!(params[:match_id])
          render_data(transaction_response)
        rescue ActiveRecord::RecordNotFound
          render_error("match_not_found", "Match not found.", status: :not_found)
        end

        # POST .../bank_transactions/:id/exclude
        def exclude
          reason = params[:reason].to_s.strip
          unless VALID_EXCLUSION_REASONS.include?(reason)
            return render_error("invalid_reason", "Invalid exclusion reason.", status: :unprocessable_entity)
          end

          Reconciliations::LineActions.new(@transaction).exclude!(reason)

          # If "loan" reason, try Loans::Matcher immediately.
          if reason == "loan"
            Loans::Matcher.new(@reconciliation).call
            @transaction.reload
          end

          render_data(transaction_response)
        rescue ArgumentError => e
          render_error("invalid_reason", e.message, status: :unprocessable_entity)
        end

        # POST .../bank_transactions/:id/reset
        def reset
          Reconciliations::LineActions.new(@transaction).reset!
          render_data(transaction_response)
        end

        # POST .../bank_transactions/:id/manual_match
        def manual_match
          doc = current_workspace.documents.find(params[:document_id])
          Reconciliations::LineActions.new(@transaction).manual_match!(doc)
          render_data(transaction_response)
        rescue ActiveRecord::RecordNotFound
          render_error("document_not_found", "Document not found.", status: :not_found)
        end

        # POST .../bank_transactions/:id/request_invoice
        # Marks the transaction as :requested and returns a draft email payload so
        # the SPA can open the compose view.
        def request_invoice
          nif_flagged = @transaction.nif_flagged?(current_workspace.company_nif.presence)
          unless @transaction.unmatched? || nif_flagged
            return render_error("not_applicable", "Transaction is not in an eligible state.", status: :unprocessable_entity)
          end

          now = ::Time.current
          if @transaction.unmatched?
            @transaction.update!(status: :requested, requested_at: now, requested_by: current_user)
          else
            @transaction.update!(requested_at: now, requested_by: current_user)
          end

          to_address = params[:to_address].to_s.strip
          subject, body = draft_invoice_request_copy(nif_flagged: nif_flagged)

          render_data({
            transaction:  BankTransactionSerializer.new(@transaction).as_json,
            draft:        { to: to_address, subject: subject, body: body },
            money:        refreshed_money_page
          })
        end

        # POST .../bank_transactions/:id/upload_and_link
        def upload_and_link
          file = params[:file]
          return render_error("no_file", "No file provided.", status: :bad_request) unless file

          document = current_workspace.documents.build(
            source:        :manual_upload,
            ai_status:     :pending,
            review_status: :pending
          )
          document.original_file.attach(file)

          unless document.save
            return render_error("upload_failed", document.errors.full_messages.to_sentence,
                                status: :unprocessable_entity)
          end

          DocumentProcessJob.perform_later(document.id)

          allocated = document.amount_cents.presence || @transaction.amount_cents.abs
          match = @transaction.transaction_matches.find_or_initialize_by(document_id: document.id)
          match.assign_attributes(
            status:          :confirmed,
            matched_by:      :manual,
            confidence:      1.0,
            match_reasons:   { "manual" => true, "uploaded" => true },
            allocated_cents: allocated
          )
          match.save!
          @transaction.update!(status: :matched)

          render_data(transaction_response)
        rescue ActiveRecord::RecordInvalid => e
          render_error("save_failed", e.message, status: :unprocessable_entity)
        end

        # GET .../bank_transactions/:id/resolve_panel
        # Returns suggested matches and candidate documents for manual matching.
        def resolve_panel
          q = params[:q].to_s.strip
          candidates_service = Reconciliations::Candidates.new(
            bank_transaction: @transaction,
            workspace:        current_workspace
          )
          all_candidates = candidates_service.call
          # Filter by q if provided (fuzzy title/counterparty match)
          candidates = if q.present?
            all_candidates.select { |h| h[:document].display_title.to_s.downcase.include?(q.downcase) }
          else
            all_candidates
          end

          render_data({
            transaction:       BankTransactionSerializer.new(@transaction).as_json,
            suggested_matches: @transaction.transaction_matches.suggested
                                           .includes(:document)
                                           .order(confidence: :desc)
                                           .map { |m| serialize_match(m) },
            candidates:        candidates.map { |h| serialize_candidate(h[:document]) }
          })
        end

        private

        def require_accounting_enabled
          render_not_found unless Features.accounting?
        end

        def require_accounting_entitlement
          require_entitlement!(:accounting)
        end

        def set_reconciliation
          @reconciliation = current_workspace.reconciliations.find(params[:reconciliation_id])
        end

        def set_transaction
          @transaction = @reconciliation.bank_transactions.find(params[:id])
        end

        def transaction_response
          @transaction.reload
          {
            transaction: BankTransactionSerializer.new(@transaction).as_json,
            money:       refreshed_money_page
          }
        end

        def refreshed_money_page
          Api::App::Money::PageSerializer.new(
            ::Money::Page.for(current_workspace, current_user,
                              today:        ::Date.current,
                              statement_id: @reconciliation.id)
          ).as_json
        end

        def draft_invoice_request_copy(nif_flagged:)
          amount_text = @transaction.signed_amount_label
          date_text   = I18n.l(@transaction.booked_on, format: :date)
          counterparty = @transaction.counterparty.presence || I18n.t("reconciliations.bank_transactions.request_invoice.counterparty_fallback")
          company_nif  = current_workspace.company_nif.presence.to_s

          if nif_flagged
            subject = I18n.t("reconciliations.bank_transactions.request_invoice.corrected_subject",
                             amount: amount_text, date: date_text)
            body = I18n.t("reconciliations.bank_transactions.request_invoice.corrected_body",
                          amount: amount_text, date: date_text,
                          counterparty: counterparty, company_nif: company_nif)
          else
            subject = I18n.t("reconciliations.bank_transactions.request_invoice.standard_subject",
                             amount: amount_text, date: date_text)
            body = I18n.t("reconciliations.bank_transactions.request_invoice.standard_body",
                          amount: amount_text, date: date_text,
                          counterparty: counterparty, company_nif: company_nif)
          end

          [ subject, body ]
        end

        def serialize_match(match)
          {
            id:             match.id,
            status:         match.status,
            confidence:     match.confidence,
            matched_by:     match.matched_by,
            document_id:    match.document_id,
            document_title: match.document&.display_title
          }
        end

        def serialize_candidate(document)
          {
            id:            document.id,
            title:         document.display_title,
            document_type: document.document_type,
            amount_cents:  document.amount_cents,
            currency:      document.currency
          }
        end
      end
    end
  end
end
