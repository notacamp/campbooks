# frozen_string_literal: true

# The Accounting / Reconciliation surface: import a bank statement (CSV or PDF
# Document), then match each transaction against workspace Documents.
#
# PR 1 scope: index / new / create / show / destroy + CSV ParseJob.
# Matching workbench, resolve actions, and zip export ship in PR 2/3.
#
# Gated by Features.accounting? (readiness) and the :accounting entitlement
# (billing). Cross-workspace access returns 404 per the app convention.
class ReconciliationsController < ApplicationController
  include ActionView::RecordIdentifier # bare dom_id(...) in turbo_stream helpers

  before_action :require_authentication
  before_action :require_accounting_enabled
  before_action :set_reconciliation, only: %i[show destroy confirm_all_suggestions export download]
  before_action :set_bank_statement_documents, only: %i[new create]

  def index
    @pagy, @reconciliations = pagy(
      Current.workspace.reconciliations.recent.includes(:statement_document),
      limit: 25
    )

    # Finding 13: batch-load transaction counts to avoid N+1 queries for the
    # progress bar on each reconciliation row/card.
    rids = @reconciliations.map(&:id)
    tx_totals    = BankTransaction.where(reconciliation_id: rids)
                                  .group(:reconciliation_id).count
    tx_resolved  = BankTransaction.where(reconciliation_id: rids,
                                         status: %i[matched excluded requested])
                                  .group(:reconciliation_id).count

    # Pre-populate the memoized ivars on each Reconciliation instance so the
    # model methods (#total_transactions / #resolved_count) skip the DB hit.
    @reconciliations.each do |r|
      r.instance_variable_set(:@total_transactions, tx_totals.fetch(r.id, 0))
      r.instance_variable_set(:@resolved_count,     tx_resolved.fetch(r.id, 0))
    end

    respond_to do |format|
      format.html
      format.turbo_stream # pagination append -> index.turbo_stream.erb
    end
  end

  def new
  end

  def create
    return if require_entitlement!(:accounting)

    statement_document = resolve_or_create_statement_document
    return if statement_document.nil? # errors already handled

    @reconciliation = Current.workspace.reconciliations.new(
      created_by:         current_user,
      statement_document: statement_document,
      bank_name:          statement_document.metadata&.dig("bank_name"),
      currency:           statement_document.currency.presence || "EUR"
    )

    if @reconciliation.save
      Reconciliations::ParseJob.perform_later(@reconciliation.id)
      redirect_to @reconciliation, success: t(".created")
    else
      render :new, status: :unprocessable_entity
    end
  end

  def show
    @pagy, @transactions = pagy(
      @reconciliation.bank_transactions.ordered
                     .includes(transaction_matches: :document),
      limit: 50
    )

    respond_to do |format|
      format.html
      format.turbo_stream
    end
  end

  # POST /reconciliations/:id/export
  # Kicks off the async zip build. Refuses (info toast) when already generating.
  def export
    return if require_entitlement!(:accounting, ignore_limit: true)

    if @reconciliation.export_generating?
      render turbo_stream: notify_stream(t(".already_generating"), severity: :info)
      return
    end

    # Reset a previous export so the user can re-export after confirming more.
    @reconciliation.update!(export_status: :export_none) if @reconciliation.export_generated?

    Reconciliations::ExportJob.perform_later(@reconciliation.id)

    render turbo_stream: [
      turbo_stream.update("reconciliation_export_button",
                          partial: "reconciliations/export_button",
                          locals: { reconciliation: @reconciliation }),
      notify_stream(t(".enqueued"), severity: :info)
    ]
  end

  # GET /reconciliations/:id/download
  # Redirects to the signed blob URL for the generated zip.
  def download
    unless @reconciliation.export_zip.attached?
      redirect_to @reconciliation, info: t(".not_ready")
      return
    end

    redirect_to rails_blob_url(@reconciliation.export_zip, disposition: "attachment"),
                allow_other_host: true
  end

  def destroy
    # Finding 10: ignore_limit: true — the :accounting entitlement may cap the
    # number of reconciliations but must never block deleting one.
    return if require_entitlement!(:accounting, ignore_limit: true)

    @reconciliation.destroy
    redirect_to accounting_path, success: t(".destroyed")
  end

  # POST /reconciliations/:id/confirm_all_suggestions
  # Bulk-confirm all highest-confidence suggested matches across the reconciliation.
  def confirm_all_suggestions
    return if require_entitlement!(:accounting, ignore_limit: true)

    suggested_txns = @reconciliation.bank_transactions
                                    .where(status: :suggested)
                                    .includes(transaction_matches: :document)

    confirmed_txns = []

    # One transaction for the whole bulk operation — atomic and concurrency-safe.
    ActiveRecord::Base.transaction do
      suggested_txns.each do |txn|
        best_match = txn.transaction_matches.select(&:suggested?)
                        .max_by(&:confidence)
        next unless best_match

        # Guard: re-check status inside transaction to avoid double-confirm.
        next unless best_match.reload.suggested?

        best_match.update!(status: :confirmed)
        txn.update!(status: :matched)
        confirmed_txns << txn
      end
    end

    all_streams = confirmed_txns.flat_map do |txn|
      txn.reload
      [
        turbo_stream.replace(dom_id(txn),        html: render_tx_row(txn)),
        turbo_stream.replace(dom_id(txn, :card), html: render_tx_card(txn))
      ]
    end

    all_streams << turbo_stream.replace("reconciliation_summary_bar",
                                        html: render_summary_bar(@reconciliation))
    all_streams << notify_stream(t(".confirmed", count: confirmed_txns.size))

    render turbo_stream: all_streams
  end

  private

  # These helpers are shared by confirm_all_suggestions only — workbench actions
  # live in Reconciliations::BankTransactionsController.

  def render_tx_row(txn)
    ApplicationController.render(partial: "bank_transactions/row",
                                 locals: { transaction: txn, reconciliation: @reconciliation },
                                 layout: false)
  end

  def render_tx_card(txn)
    ApplicationController.render(partial: "bank_transactions/card",
                                 locals: { transaction: txn, reconciliation: @reconciliation },
                                 layout: false)
  end

  def render_summary_bar(reconciliation)
    counts    = reconciliation.bank_transactions.group(:status).count
    nif_count = reconciliation.nif_exception_count(Current.workspace.company_nif.presence)
    ApplicationController.render(
      partial: "reconciliations/summary_bar",
      locals:  { reconciliation: reconciliation, status_counts: counts, nif_exception_count: nif_count },
      layout:  false
    )
  end

  def set_reconciliation
    @reconciliation = Current.workspace.reconciliations.find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  # Finding 16: shared before_action so both :new and the :create error path
  # use the same query without duplicating it.
  def set_bank_statement_documents
    @bank_statement_documents = Current.workspace.documents
                                       .where(document_type: :bank_statement)
                                       .order(created_at: :desc)
                                       .limit(50)
  end

  # Either find an existing Document by statement_document_id, or create one
  # from an uploaded CSV/PDF file.  Returns nil and responds on error.
  #
  # Absolute i18n keys are used here because lazy keys in private methods would
  # resolve to the method name rather than the calling action's name.
  def resolve_or_create_statement_document
    if params[:statement_document_id].present?
      doc = Current.workspace.documents.find_by(id: params[:statement_document_id])
      unless doc
        flash[:error] = t("reconciliations.create.document_not_found")
        redirect_to new_reconciliation_path
        return nil
      end
      doc
    elsif params[:statement_file].present?
      file = params[:statement_file]
      doc = Current.workspace.documents.new(
        source:        :manual_upload,
        document_type: :bank_statement,
        ai_status:     :skipped,
        review_status: :pending
      )
      doc.original_file.attach(file)
      unless doc.save
        flash[:error] = t("reconciliations.create.file_upload_failed")
        redirect_to new_reconciliation_path
        return nil
      end
      doc
    else
      flash[:error] = t("reconciliations.create.no_source")
      redirect_to new_reconciliation_path
      nil
    end
  end
end
