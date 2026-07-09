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
  before_action :require_authentication
  before_action :require_accounting_enabled
  before_action :set_reconciliation, only: %i[show destroy]

  def index
    @pagy, @reconciliations = pagy(
      Current.workspace.reconciliations.recent.includes(:statement_document),
      limit: 25
    )
  end

  def new
    # Recent bank_statement documents to pre-fill the picker (limit 50 for UX).
    @bank_statement_documents = Current.workspace.documents
                                       .where(document_type: :bank_statement)
                                       .order(created_at: :desc)
                                       .limit(50)
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
      @bank_statement_documents = Current.workspace.documents
                                         .where(document_type: :bank_statement)
                                         .order(created_at: :desc)
                                         .limit(50)
      render :new, status: :unprocessable_entity
    end
  end

  def show
    @pagy, @transactions = pagy(
      @reconciliation.bank_transactions.ordered
                     .includes(transaction_matches: :document),
      limit: 50
    )
  end

  def destroy
    return if require_entitlement!(:accounting)

    @reconciliation.destroy
    redirect_to accounting_path, success: t(".destroyed")
  end

  private

  def set_reconciliation
    @reconciliation = Current.workspace.reconciliations.find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
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
