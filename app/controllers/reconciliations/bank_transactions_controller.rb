# frozen_string_literal: true

module Reconciliations
  # Workbench actions on individual BankTransactions:
  # confirm, reject, exclude, reset, manual_match, and resolve_panel (lazy frame).
  #
  # All mutations respond with turbo_stream: replace both the row and card
  # (dual-DOM so desktop table and mobile card list stay in sync), replace the
  # summary bar, and flash a toast. Undo points back to `reset` where applicable.
  #
  # Workspace-scoped on every lookup: a cross-workspace BankTransaction 404s.
  class BankTransactionsController < ApplicationController
    include ActionView::RecordIdentifier # bare dom_id(...) in turbo_stream helpers

    before_action :require_authentication
    before_action :require_accounting_enabled
    before_action :set_reconciliation
    before_action :set_transaction
    before_action -> { require_entitlement!(:accounting, ignore_limit: true) },
                  only: %i[confirm reject exclude reset manual_match resolve_panel request_invoice]

    VALID_EXCLUSION_REASONS = %w[bank_fee salary transfer tax other].freeze

    # POST /reconciliations/:reconciliation_id/bank_transactions/:id/confirm
    def confirm
      match = @transaction.transaction_matches.find(params[:match_id])

      ActiveRecord::Base.transaction do
        match.update!(status: :confirmed)
        @transaction.update!(status: :matched)
      end

      render_workbench_streams(notify: t(".confirmed"), undo_url: reset_reconciliation_bank_transaction_path(@reconciliation, @transaction))
    end

    # POST /reconciliations/:reconciliation_id/bank_transactions/:id/reject
    def reject
      match = @transaction.transaction_matches.find(params[:match_id])
      match.update!(status: :rejected)

      remaining = @transaction.transaction_matches.suggested
      new_status = remaining.any? ? :suggested : :unmatched
      @transaction.update!(status: new_status)

      render_workbench_streams(notify: t(".rejected"))
    end

    # POST /reconciliations/:reconciliation_id/bank_transactions/:id/exclude
    def exclude
      reason = params[:reason].to_s.strip
      unless VALID_EXCLUSION_REASONS.include?(reason)
        render turbo_stream: notify_stream(t(".invalid_reason"), severity: :error), status: :unprocessable_entity
        return
      end

      @transaction.update!(status: :excluded, exclusion_reason: reason)

      render_workbench_streams(notify: t(".excluded", reason: human_exclusion_reason(reason)),
                               undo_url: reset_reconciliation_bank_transaction_path(@reconciliation, @transaction))
    end

    # POST /reconciliations/:reconciliation_id/bank_transactions/:id/reset
    def reset
      ActiveRecord::Base.transaction do
        # Confirmed matches → rejected; suggested matches stay
        @transaction.transaction_matches.confirmed.update_all(status: TransactionMatch.statuses[:rejected])
        @transaction.update!(
          status:          :unmatched,
          exclusion_reason: nil,
          requested_at:    nil,
          requested_by:    nil
        )

        # If any suggested matches remain, promote the transaction back to :suggested
        if @transaction.transaction_matches.reload.suggested.any?
          @transaction.update!(status: :suggested)
        end
      end

      render_workbench_streams(notify: t(".reset"))
    end

    # POST /reconciliations/:reconciliation_id/bank_transactions/:id/manual_match
    def manual_match
      doc = Current.workspace.documents.find(params[:document_id])

      match = @transaction.transaction_matches.find_or_initialize_by(document_id: doc.id)
      match.assign_attributes(
        status:        :confirmed,
        matched_by:    :manual,
        confidence:    1.0,
        match_reasons: { "manual" => true }
      )
      match.save!
      @transaction.update!(status: :matched)

      render_workbench_streams(notify: t(".matched", title: doc.display_title),
                               undo_url: reset_reconciliation_bank_transaction_path(@reconciliation, @transaction))
    rescue ActiveRecord::RecordNotFound
      render turbo_stream: notify_stream(t(".document_not_found"), severity: :error), status: :not_found
    end

    # POST /reconciliations/:reconciliation_id/bank_transactions/:id/request_invoice
    # One-click "ask for invoice" flow. Marks the transaction as :requested,
    # then opens the Dock composer prefilled with a template email to the
    # counterparty. Nothing is sent automatically.
    #
    # Two cases:
    #   - Unmatched transaction: set status :requested + requested_at/by
    #   - Matched transaction with NIF-flagged document: keep status :matched,
    #     just record requested_at/by (the "corrected invoice" case)
    def request_invoice
      nif_flagged = nif_flagged_match?
      unless @transaction.unmatched? || nif_flagged
        render turbo_stream: notify_stream(t(".not_applicable"), severity: :info)
        return
      end

      now = Time.current
      if @transaction.unmatched?
        @transaction.update!(status: :requested, requested_at: now, requested_by: current_user)
      else
        # Corrected-invoice case: keep matched status, just record the chase timestamp
        @transaction.update!(requested_at: now, requested_by: current_user)
      end

      to_address    = params[:to_address].to_s.strip
      subject_key   = nif_flagged ? ".corrected_subject" : ".standard_subject"
      body_key      = nif_flagged ? ".corrected_body"    : ".standard_body"
      company_nif   = Current.workspace.company_nif.presence

      subject = t(subject_key,
                  amount: invoice_amount_text,
                  date:   l(@transaction.booked_on, format: :date))
      body = t(body_key,
               amount:      invoice_amount_text,
               date:        l(@transaction.booked_on, format: :date),
               counterparty: @transaction.counterparty.presence || t(".counterparty_fallback"),
               company_nif: company_nif.to_s)

      dock_locals = EmailCompose::DockLocals.blank(
        user:    Current.user,
        to:      to_address,
        subject: subject,
        body:    body
      )

      row_html     = render_row_html(@transaction)
      card_html    = render_card_html(@transaction)
      summary_html = render_summary_bar_html

      render turbo_stream: [
        turbo_stream.replace(dom_id(@transaction),        html: row_html),
        turbo_stream.replace(dom_id(@transaction, :card), html: card_html),
        # Remove stale resolve panel if open
        turbo_stream.remove("#{dom_id(@transaction)}_resolve_panel"),
        turbo_stream.remove("#{dom_id(@transaction, :card)}_resolve_panel"),
        turbo_stream.replace("reconciliation_summary_bar", html: summary_html),
        turbo_stream.update("compose_dock",
                             partial: "email_compose/dock",
                             locals:  dock_locals),
        notify_stream(t(".opened_composer"))
      ]
    end

    # GET /reconciliations/:reconciliation_id/bank_transactions/:id/resolve_panel
    # Also handles doc-search within the panel (params[:q] filters candidates).
    def resolve_panel
      @q                   = params[:q].to_s.strip
      @suggested_matches   = @transaction.transaction_matches.suggested
                                         .includes(:document)
                                         .order(confidence: :desc)
      @candidate_documents = candidate_documents_for(@transaction, q: @q)
      @company_nif         = Current.workspace.company_nif.presence

      respond_to do |format|
        format.html
        format.turbo_stream # doc-search refinement updates the inner frame only
      end
    end

    private

    def set_reconciliation
      @reconciliation = Current.workspace.reconciliations.find(params[:reconciliation_id])
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    def set_transaction
      @transaction = @reconciliation.bank_transactions.find(params[:id])
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    # Build a single turbo_stream response that:
    #   - replaces the table row (desktop)
    #   - replaces the card (mobile)
    #   - replaces the summary bar
    #   - appends a toast notification (with optional undo link)
    def render_workbench_streams(notify:, undo_url: nil)
      @transaction.reload
      @transaction.transaction_matches.reload

      row_html     = render_row_html(@transaction)
      card_html    = render_card_html(@transaction)
      summary_html = render_summary_bar_html

      toast_message = undo_url.present? ?
        "#{notify} &mdash; <a class='font-medium underline' data-turbo-method='post' href='#{undo_url}'>#{t("shared.actions.undo")}</a>".html_safe :
        notify

      render turbo_stream: [
        turbo_stream.replace(dom_id(@transaction),        html: row_html),
        turbo_stream.replace(dom_id(@transaction, :card), html: card_html),
        # Remove the stale resolve panel sibling row/div if still open
        turbo_stream.remove("#{dom_id(@transaction)}_resolve_panel"),
        turbo_stream.remove("#{dom_id(@transaction, :card)}_resolve_panel"),
        turbo_stream.replace("reconciliation_summary_bar", html: summary_html),
        notify_stream(toast_message)
      ]
    end

    def render_row_html(txn)
      ApplicationController.render(
        partial: "bank_transactions/row",
        locals:  { transaction: txn, reconciliation: @reconciliation },
        layout:  false
      )
    end

    def render_card_html(txn)
      ApplicationController.render(
        partial: "bank_transactions/card",
        locals:  { transaction: txn, reconciliation: @reconciliation },
        layout:  false
      )
    end

    def render_summary_bar_html
      counts    = @reconciliation.bank_transactions.group(:status).count
      nif_count = @reconciliation.nif_exception_count(Current.workspace.company_nif.presence)
      ApplicationController.render(
        partial: "reconciliations/summary_bar",
        locals:  { reconciliation: @reconciliation, status_counts: counts, nif_exception_count: nif_count },
        layout:  false
      )
    end

    def candidate_documents_for(txn, q: nil)
      scope = Current.workspace.documents
                     .where(document_type: txn.candidate_document_types)
                     .where.not(amount_cents: [ nil, 0 ])
                     .order(document_date: :desc)
                     .limit(30)

      if q.present?
        like = "%#{ActiveRecord::Base.sanitize_sql_like(q)}%"
        scope = scope.where(
          "vendor_name ILIKE :q OR sender_name ILIKE :q OR client_name ILIKE :q " \
          "OR invoice_number ILIKE :q OR description ILIKE :q",
          q: like
        )
      end

      scope
    end

    def human_exclusion_reason(reason)
      I18n.t("reconciliations.bank_transactions.exclusion_reasons.#{reason}")
    end

    # True when the transaction is matched and the top confirmed document has a
    # NIF issue (missing or mismatch) — the "corrected invoice" case.
    def nif_flagged_match?
      return false unless @transaction.matched?

      company_nif = Current.workspace.company_nif.presence
      return false if company_nif.blank?

      top_match = @transaction.transaction_matches.select(&:confirmed?).max_by(&:confidence)
      return false unless top_match

      top_match.document.nif_status(company_nif)&.in?(%i[missing mismatch]) || false
    end

    def invoice_amount_text
      sign = @transaction.debit? ? "-" : "+"
      amt  = format("%.2f", @transaction.amount_cents.abs / 100.0)
      "#{sign}#{amt} #{@transaction.currency}"
    end
  end
end
