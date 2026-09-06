# frozen_string_literal: true

# CRUD for tracked Loans. Lives under the Money surface; subject to the same
# accounting feature gate and :accounting entitlement as MoneyController.
class LoansController < ApplicationController
  before_action :require_accounting_enabled
  before_action :require_entitlement!, only: %i[create update destroy show]
  before_action :set_loan, only: %i[update destroy show]

  # POST /money/loans
  def create
    @loan = Current.workspace.loans.build(loan_params)
    @loan.created_by = current_user

    if @loan.save
      Loans::Schedule.build!(@loan)
      Loans::Status.refresh!(@loan)
      backfill_result = Loans::Backfill.call(@loan)
      @loan.reload

      linked_count = @loan.instalments.where(status: %i[paid unverified]).count
      message = if linked_count > 0
                  t(".created_with_count", count: linked_count)
                else
                  t(".created_no_match")
                end

      respond_action(message)
    else
      render turbo_stream: notify_stream(@loan.errors.full_messages.first, severity: :error),
             status: :unprocessable_entity
    end
  end

  # PATCH /money/loans/:id
  def update
    if @loan.update(loan_params)
      Loans::Schedule.rebuild!(@loan)
      Loans::Backfill.call(@loan)
      respond_action(t(".updated"))
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

  # GET /money/loans/:id
  def show
    @instalments = @loan.instalments.includes(:bank_transaction).ordered
  end

  # POST /money/loans/dismiss
  def dismiss
    key = params[:key].to_s.strip
    if key.present?
      settings = Current.workspace.settings.dup
      dismissed = Array(settings["dismissed_loan_suggestions"])
      dismissed << key unless dismissed.include?(key)
      settings["dismissed_loan_suggestions"] = dismissed
      Current.workspace.update_columns(settings: settings)
    end

    respond_action(t(".dismissed"))
  end

  private

  def require_entitlement!
    super(:accounting)
  end

  def set_loan
    @loan = Current.workspace.loans.find(params[:id])
  rescue ActiveRecord::RecordNotFound
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(t("shared.not_found"), severity: :error), status: :not_found }
      format.html         { head :not_found }
    end
  end

  def loan_params
    raw = params.require(:loan).permit(
      :lender, :source_counterparty, :principal, :instalment,
      :first_instalment_on, :term_months, :rate_note, :notes
    )

    raw[:principal_cents]  = parse_money_cents(raw.delete(:principal))  if raw.key?(:principal)
    raw[:instalment_cents] = parse_money_cents(raw.delete(:instalment)) if raw.key?(:instalment)
    raw
  end

  # Parse monetary input in various formats: "46.800,00", "46800", "€46,800"
  def parse_money_cents(value)
    return 0 if value.blank?

    clean = value.to_s
                 .gsub(/[€$£R$]/i, "")   # strip currency symbols
                 .gsub(/\s/, "")

    # Detect European format "46.800,00" vs "46,800.00" vs bare "46800"
    if clean.match?(/\d+\.\d{3},\d{2}$/)
      # European: 46.800,00
      clean = clean.gsub(".", "").gsub(",", ".")
    elsif clean.match?(/\d+,\d{3}\.\d{2}$/)
      # US: 46,800.00
      clean = clean.gsub(",", "")
    elsif clean.include?(",") && !clean.include?(".")
      # Bare comma decimal: "780,00"
      clean = clean.gsub(",", ".")
    else
      # Bare dot or plain integer
      clean = clean.gsub(",", "")
    end

    (clean.to_f * 100).round
  end

  def respond_action(message)
    build_ledger_data
    render turbo_stream: [
      turbo_stream.replace("money_content", partial: "money/content",
                           locals: money_content_locals),
      notify_stream(message)
    ]
  rescue => e
    Rails.logger.warn("[LoansController] respond_action failed: #{e.class}: #{e.message}")
    render turbo_stream: notify_stream(message)
  end

  def build_ledger_data
    @today   = Date.current
    @loans   = Current.workspace.loans.active_loans.includes(instalments: :bank_transaction)
                      .order(:created_at)
    @loan_suggestions = Loans::Spotter.new(Current.workspace).call
  end

  def money_content_locals
    {}
  end
end
