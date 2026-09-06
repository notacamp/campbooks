# frozen_string_literal: true

require "csv"

# The Money surface: bank reconciliation answered to the paper behind it.
# Scout's read, a Needs-you list, the paired statement ledger, and the
# "Not on a statement" list of documents with no bank line. Gated by the
# accounting flag and entitlement.
class MoneyController < ApplicationController
  before_action :require_accounting_enabled
  before_action :require_accounting_entitlement
  before_action :set_obligation, only: %i[chase settle unsettle]

  def index
    @page = Money::Page.for(Current.workspace, current_user,
                            today:        Date.current,
                            statement_id: params[:statement])
  end

  # GET /money/statement/:id — renders ONLY the money_statement turbo-frame content
  # for that reconciliation. 404 when the id belongs to another workspace.
  def statement
    recon = Current.workspace.reconciliations.ready.find_by!(id: params[:id])
    @page = Money::Page.for(Current.workspace, current_user,
                            today:        Date.current,
                            statement_id: recon.id)
    render partial: "money/statement_frame", locals: { page: @page }, layout: false
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  # POST /money/statements/reconcile — every bank statement Scout already holds
  # that nobody reconciled starts reconciling in the background.
  def reconcile_statements
    documents = Reconciliations::AutoStart.pending_for(Current.workspace).to_a
    documents.each { |doc| Reconciliations::AutoStartJob.perform_later(doc.id, created_by_id: current_user.id) }
    respond_action(t("money.actions.reconciling", count: documents.size))
  end

  # GET /money/statements — the full bank-statement reconciliation list.
  def statements
    @pagy, @reconciliations = pagy(
      Current.workspace.reconciliations.recent.includes(:statement_document),
      limit: 25
    )

    rids        = @reconciliations.map(&:id)
    tx_totals   = BankTransaction.where(reconciliation_id: rids)
                                 .group(:reconciliation_id).count
    tx_resolved = BankTransaction.where(reconciliation_id: rids,
                                        status: BankTransaction::RESOLVED_STATUSES)
                                 .group(:reconciliation_id).count

    @reconciliations.each do |r|
      r.instance_variable_set(:@total_transactions, tx_totals.fetch(r.id, 0))
      r.instance_variable_set(:@resolved_count,     tx_resolved.fetch(r.id, 0))
    end

    respond_to do |format|
      format.html
      format.turbo_stream
    end
  end

  # GET /money/export — CSV of the ledger for your accountant.
  def export
    if (recon = ready_reconciliation_for_quarter)
      redirect_to download_reconciliation_path(recon) and return
    end

    ledger = Money::Ledger.for(Current.workspace, current_user, today: Date.current)
    send_data ledger_csv(ledger.obligations),
              filename: "#{t('money.export.filename', quarter: quarter_label)}.csv",
              type: "text/csv"
  end

  # POST /money/obligations/:id/chase — open the compose Dock with a chase draft.
  # Only for missing receivables.
  def chase
    return respond_gone unless @obligation&.missing?
    return respond_gone unless @obligation.receivable?

    draft = Money::ReminderDraft.chase(@obligation)
    open_dock(draft, t("money.actions.reminder_opened"))
  end

  # POST /money/obligations/:id/settle — manual settlement.
  def settle
    return respond_gone unless @obligation&.document

    source = params[:source].to_s.presence || "manual"
    @obligation.document.mark_settled!(source: source)
    respond_action(nil, undo: undo_toast(t("money.actions.marked_paid"),
                                          endpoint: money_obligation_settle_path(@obligation.id),
                                          method: :delete))
  end

  # DELETE /money/obligations/:id/settle — undo.
  def unsettle
    return respond_gone unless @obligation&.document

    @obligation.document.mark_unsettled!
    respond_action(t("money.actions.marked_unpaid"))
  end

  # POST /money/lines/:id/confirm — confirm a suggested match from Money.
  def confirm_line
    txn = workspace_transaction
    return unless txn

    begin
      Reconciliations::LineActions.new(txn).confirm!(params[:match_id])
      respond_action(t("money.actions.line_confirmed"),
                     undo: undo_line_toast(txn, t("money.actions.line_confirmed")))
    rescue ActiveRecord::RecordNotFound
      respond_action(t("money.actions.line_not_found"), severity: :error)
    end
  end

  # POST /money/lines/:id/set_aside — exclude a transaction from Money.
  def set_aside_line
    txn = workspace_transaction
    return unless txn

    reason = params[:reason].to_s.strip
    unless Reconciliations::BankTransactionsController::VALID_EXCLUSION_REASONS.include?(reason)
      respond_to do |format|
        format.turbo_stream do
          render turbo_stream: notify_stream(t("money.actions.invalid_reason"), severity: :error),
                 status: :unprocessable_entity
        end
        format.any { head :unprocessable_entity }
      end
      return
    end

    Reconciliations::LineActions.new(txn).exclude!(reason)
    respond_action(t("money.actions.line_excluded"),
                   undo: undo_line_toast(txn, t("money.actions.line_excluded")))
  end

  # POST /money/lines/:id/reset — undo a line action.
  def reset_line
    txn = workspace_transaction
    return unless txn

    Reconciliations::LineActions.new(txn).reset!
    respond_action(t("money.actions.line_reset"))
  end

  private

  def require_accounting_entitlement
    require_entitlement!(:accounting)
  end

  def build_page(statement_id: params[:statement])
    Money::Page.for(Current.workspace, current_user,
                    today:        Date.current,
                    statement_id: statement_id)
  end

  def set_obligation
    @page = build_page
    evidence = @page.evidence
    ledger   = @page.ledger
    @obligation = ledger.find(params[:id])
    respond_gone unless @obligation
  end

  def workspace_transaction
    txn = BankTransaction.joins(:reconciliation)
                         .where(reconciliations: { workspace_id: Current.workspace.id })
                         .find_by(id: params[:id])
    unless txn
      respond_to do |format|
        format.turbo_stream { render turbo_stream: notify_stream(t("money.actions.gone"), severity: :warning), status: :not_found }
        format.any { head :not_found }
      end
    end
    txn
  end

  # Re-render the money_content region and raise a toast.
  def respond_action(message, undo: nil, extra: [], severity: :success, statement_id: nil)
    page = build_page(statement_id: statement_id || params[:statement])
    streams = [ turbo_stream.replace("money_content", partial: "money/content", locals: { page: page }) ]
    streams << (undo || notify_stream(message, severity: severity)) if undo || message.present?
    streams += Array(extra)

    respond_to do |format|
      format.turbo_stream { render turbo_stream: streams }
      format.any { redirect_to money_path }
    end
  end

  def open_dock(draft, message)
    to = chase_recipient(@obligation)
    dock = turbo_stream.update("compose_dock", partial: "email_compose/dock",
                                               locals: EmailCompose::DockLocals.blank(
                                                 user: current_user, to: to,
                                                 subject: draft.subject, body: draft.body
                                               ))
    respond_to do |format|
      format.turbo_stream { render turbo_stream: [ dock, notify_stream(message) ] }
      format.any { redirect_to money_path }
    end
  end

  def respond_gone
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: notify_stream(t("money.actions.gone"), severity: :warning),
               status: :not_found
      end
      format.any { head :not_found }
    end
  end

  def chase_recipient(obligation)
    contact = obligation.source_email_message&.contact
    contact ||= Current.workspace.contacts
                       .where("lower(name) = ?", obligation.counterpart.to_s.downcase).first
    contact&.email.to_s
  end

  def undo_toast(message, endpoint:, method: nil)
    params = {}
    params["_method"] = method.to_s if method
    turbo_stream.append(
      Campbooks::ActionToast::REGION_ID,
      render_to_string(Campbooks::ActionToast.new(
                         message: message, variant: :success,
                         undo: { endpoint: endpoint, params: params, label: t("money.actions.undo") }
                       ), layout: false)
    )
  end

  def undo_line_toast(txn, message)
    undo_toast(message,
               endpoint: reset_line_money_path(txn.id),
               method: :post)
  end

  def ledger_csv(obligations)
    headers = %i[counterpart what direction amount currency date status settled_on statement source]
    CSV.generate do |csv|
      csv << headers.map { |h| t("money.export.headers.#{h}") }
      obligations.each do |o|
        csv << [
          o.counterpart,
          o.what,
          t("money.export.direction.#{o.direction}"),
          o.amount&.amount&.to_s("F"),
          o.currency,
          o.anchor_on&.iso8601,
          t("money.export.status.#{o.status}"),
          o.settled_on&.iso8601,
          o.statement_label,
          o.document ? "document:#{o.document.id}" : nil
        ]
      end
    end
  end

  def quarter_label(date = Date.current)
    "Q#{((date.month - 1) / 3) + 1}"
  end

  def ready_reconciliation_for_quarter
    q     = (Date.current.month - 1) / 3
    range = Date.current.beginning_of_year.advance(months: q * 3).all_quarter
    Current.workspace.reconciliations.where(status: :ready)
           .where("period_start <= ? AND period_end >= ?", range.end, range.begin)
           .detect { |r| r.export_zip.attached? }
  rescue StandardError
    nil
  end
end
