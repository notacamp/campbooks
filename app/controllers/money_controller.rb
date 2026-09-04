# frozen_string_literal: true

require "csv"

# The Money surface: what you're owed, what you owe, and what the bank settled, on
# one 30-day timeline. It reads Money::Ledger over the accounting substrate; the
# row/card actions delegate to the canonical mutations (Document#mark_settled!,
# Reminders::Confirm, a manual Reminder, the compose Dock) and re-render the page's
# derived state (Scout's read, the timeline, the ledger) so totals and sections
# stay honest after every action.
#
# Money exists only where accounting does — gated by the same accounting flag +
# entitlement as the reconciliation page.
class MoneyController < ApplicationController
  before_action :require_accounting_enabled
  before_action :require_accounting_entitlement
  before_action :set_obligation, only: %i[remind chase settle unsettle decide]

  def index
    build_ledger
  end

  # A CSV of the ledger for your accountant (counterpart, what, direction, amount,
  # currency, due, status, settled_on, source). If a reconciliation for the quarter
  # is already exported, hand over that richer ZIP instead.
  def export
    if (recon = ready_reconciliation_for_quarter)
      redirect_to download_reconciliation_path(recon) and return
    end

    ledger = Money::Ledger.for(Current.workspace, current_user, today: Date.current)
    send_data ledger_csv(ledger.obligations),
              filename: "#{t('money.export.filename', quarter: quarter_label)}.csv",
              type: "text/csv"
  end

  # "Remind on <date>" — a due receivable you want to chase later. Sets a user-made
  # payment_due reminder for the day after it falls due. No email.
  def remind
    due = @obligation.due_on || Date.current
    reminder = Reminder.create!(
      workspace: Current.workspace, source: @obligation.document,
      reminder_type: :payment_due, status: :pending,
      title: I18n.t("money.reminder_draft.chase.subject_generic"),
      amount_cents: @obligation.amount_cents, currency: @obligation.currency,
      due_at: (due + 1.day).in_time_zone, all_day: true,
      extracted_data: { "origin" => "money_manual" }
    )
    respond_action(t("money.actions.reminder_created", date: l(reminder.due_at.to_date, format: :day_month)))
  end

  # "Send reminder" (a late receivable) — open the compose Dock with the chase draft
  # prefilled. Also serves a renewal's "Cancel it" cancellation draft (choice=cancel
  # routes here after dismissing). Nothing is sent; the draft is the user's to edit.
  def chase
    draft = Money::ReminderDraft.chase(@obligation)
    open_dock(draft, t("money.actions.reminder_opened"))
  end

  # "Mark paid" — a manual settlement (the bank match, when it lands, still wins).
  def settle
    return respond_gone unless @obligation.document

    @obligation.document.mark_settled!
    respond_action(nil, undo: undo_toast(t("money.actions.marked_paid"),
                                          endpoint: money_obligation_settle_path(@obligation.id), method: :delete))
  end

  def unsettle
    return respond_gone unless @obligation.document

    @obligation.document.mark_unsettled!
    respond_action(t("money.actions.marked_unpaid"))
  end

  # A renewal decision. Keep → confirm the reminder (a calendar event). Cancel it →
  # dismiss the reminder AND open a cancellation draft in the Dock.
  def decide
    return respond_gone unless @obligation.reminder

    if params[:choice] == "cancel"
      @obligation.reminder.dismissed!
      Events.publish("reminder.dismissed", subject: @obligation.reminder,
                                            payload: { "title" => @obligation.reminder.title })
      open_dock(Money::ReminderDraft.cancellation(@obligation), t("money.actions.cancelled"))
    else
      Reminders::Confirm.call(@obligation.reminder, user: current_user)
      respond_action(t("money.actions.kept"))
    end
  end

  private

  def require_accounting_entitlement
    require_entitlement!(:accounting)
  end

  def build_ledger
    @today = Date.current
    @horizon = params[:range] == "90d" ? 90.days : 30.days
    @ledger = Money::Ledger.for(Current.workspace, current_user, today: @today, horizon: @horizon)
    @summary = Money::Summary.for(Current.workspace, current_user, today: @today, ledger: @ledger)
    @quarter_label = quarter_label
  end

  def set_obligation
    ledger = Money::Ledger.for(Current.workspace, current_user, today: Date.current, horizon: 90.days)
    @obligation = ledger.find(params[:id])
    respond_gone unless @obligation
  end

  # Perform a mutation, then re-render the whole money region (Scout read + timeline
  # + ledger) so every derived figure stays correct, and raise a toast.
  def respond_action(message, undo: nil, extra: [])
    build_ledger
    streams = [ turbo_stream.replace("money_content", partial: "money/content") ]
    streams << (undo || notify_stream(message)) if undo || message.present?
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
                                                 user: current_user, to: to, subject: draft.subject, body: draft.body
                                               ))
    # Cancelling a renewal also dismisses it, so refresh the ledger region too.
    if params[:choice] == "cancel"
      respond_action(message, extra: [ dock ])
    else
      respond_to do |format|
        format.turbo_stream { render turbo_stream: [ dock, notify_stream(message) ] }
        format.any { redirect_to money_path }
      end
    end
  end

  def respond_gone
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(t("money.actions.gone"), severity: :warning), status: :not_found }
      format.any { head :not_found }
    end
  end

  # Best-effort recipient for a chase/cancellation: the source email's contact, else
  # a workspace contact whose name matches the counterpart. Blank is fine — the Dock
  # is editable, and Money never sends on its own.
  def chase_recipient(obligation)
    contact = obligation.source_email_message&.contact
    contact ||= Current.workspace.contacts.where("lower(name) = ?", obligation.counterpart.to_s.downcase).first
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

  def ledger_csv(obligations)
    headers = %i[counterpart what direction amount currency due status settled_on source]
    CSV.generate do |csv|
      csv << headers.map { |h| t("money.export.headers.#{h}") }
      obligations.each do |o|
        csv << [
          o.counterpart, o.what, t("money.export.direction.#{o.direction}"),
          o.amount&.amount&.to_s("F"), o.currency, o.due_on&.iso8601,
          t("money.export.status.#{o.status}"), o.settled_on&.iso8601, csv_source(o)
        ]
      end
    end
  end

  def csv_source(obligation)
    return obligation.settled_via if obligation.settled_via.present?

    obligation.document ? "document:#{obligation.document.id}" : "reminder:#{obligation.reminder&.id}"
  end

  def quarter_label(date = Date.current)
    "Q#{((date.month - 1) / 3) + 1}"
  end

  def ready_reconciliation_for_quarter
    q = (Date.current.month - 1) / 3
    range = Date.current.beginning_of_year.advance(months: q * 3).all_quarter
    Current.workspace.reconciliations.where(status: :ready)
           .where("period_start <= ? AND period_end >= ?", range.end, range.begin)
           .detect { |r| r.export_zip.attached? }
  rescue StandardError
    nil
  end
end
