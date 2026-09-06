# frozen_string_literal: true

# Tracked loans, under the Money surface: create from Scout's suggestion (or by
# hand), edit the terms, stop tracking, the full instalment list, and dismissing a
# suggestion. Same accounting gate and entitlement as MoneyController. Every
# mutation re-renders #money_content from Money::Page so the page stays honest.
class LoansController < ApplicationController
  before_action :require_accounting_enabled
  before_action :require_accounting_entitlement
  before_action :set_loan, only: %i[update destroy show]

  # POST /money/loans
  def create
    @loan = Current.workspace.loans.build(loan_params)
    @loan.created_by = current_user

    if @loan.save
      Loans::Schedule.build!(@loan)
      Loans::Backfill.call(@loan)
      linked = @loan.instalments.where(status: :paid).count
      respond_action(linked.positive? ? t(".created_with_count", count: linked) : t(".created_no_match"))
    else
      render turbo_stream: notify_stream(@loan.errors.full_messages.first, severity: :error),
             status: :unprocessable_entity
    end
  end

  # PATCH /money/loans/:id
  def update
    if @loan.update(loan_params)
      Loans::Schedule.rebuild!(@loan) if @loan.saved_changes.keys.intersect?(%w[term_months first_instalment_on instalment_cents])
      Loans::Backfill.call(@loan)
      respond_action(@loan.saved_changes.key?("change_acknowledged_at") ? t(".acknowledged") : t(".updated"))
    else
      render turbo_stream: notify_stream(@loan.errors.full_messages.first, severity: :error),
             status: :unprocessable_entity
    end
  end

  # DELETE /money/loans/:id
  def destroy
    @loan.destroy!
    respond_action(t(".destroyed"))
  end

  # GET /money/loans/:id — the full schedule
  def show
    @instalments = @loan.instalments.includes(bank_transaction: :reconciliation).ordered
  end

  # POST /money/loans/dismiss — "Not a loan"
  def dismiss
    key = params[:key].to_s.strip
    if key.present?
      settings  = Current.workspace.settings.deep_dup
      dismissed = Array(settings["dismissed_loan_suggestions"])
      dismissed << key unless dismissed.include?(key)
      settings["dismissed_loan_suggestions"] = dismissed
      Current.workspace.update!(settings: settings)
    end

    respond_action(t(".dismissed"))
  end

  private

  def require_accounting_entitlement
    require_entitlement!(:accounting)
  end

  def set_loan
    @loan = Current.workspace.loans.find(params[:id])
  rescue ActiveRecord::RecordNotFound
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(t("money.actions.gone"), severity: :warning), status: :not_found }
      format.any          { head :not_found }
    end
  end

  def loan_params
    raw = params.require(:loan).permit(
      :lender, :source_counterparty, :principal, :instalment,
      :first_instalment_on, :term_months, :rate_note, :notes, :change_acknowledged_at
    )
    raw[:principal_cents]  = parse_money_cents(raw.delete(:principal))  if raw.key?(:principal)
    raw[:instalment_cents] = parse_money_cents(raw.delete(:instalment)) if raw.key?(:instalment)
    raw
  end

  # "46.800,00", "46,800.00", "46.800", "46,800", "780,00", "€46,800", "46800".
  def parse_money_cents(value)
    return 0 if value.blank?

    clean = value.to_s.gsub(/[€$£]|R\$/, "").gsub(/\s/, "")
    clean =
      if clean.match?(/\A\d{1,3}(\.\d{3})+(,\d{1,2})?\z/)      # European: 46.800 or 46.800,00
        clean.delete(".").tr(",", ".")
      elsif clean.match?(/\A\d{1,3}(,\d{3})+(\.\d{1,2})?\z/)   # English: 46,800 or 46,800.00
        clean.delete(",")
      elsif clean.include?(",") && !clean.include?(".")         # 780,00
        clean.tr(",", ".")
      else
        clean.delete(",")
      end

    (clean.to_f * 100).round
  end

  def respond_action(message)
    page = Money::Page.for(Current.workspace, current_user, today: Date.current, statement_id: params[:statement])
    render turbo_stream: [
      turbo_stream.replace("money_content", partial: "money/content", locals: { page: page }),
      notify_stream(message)
    ]
  end
end
