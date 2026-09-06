# frozen_string_literal: true

# Lookbook previews for the five Money loan components:
#   LoanCard, LoanSuggestionRow, LoanForm, LoanAlertRow, LoanStat
#
# All previews build in-memory stubs so no database is required.
class MoneyLoanComponentsPreview < ViewComponent::Preview
  # -------------------------------------------------------------------
  # LoanCard
  # -------------------------------------------------------------------

  # Active loan — on schedule, 18 of 60 paid, next instalment upcoming.
  # @label LoanCard (active, on schedule)
  def loan_card_active
    render Campbooks::Money::LoanCard.new(loan: build_loan)
  end

  # Active loan with a rate-reset instalment (ring on tick 6).
  # @label LoanCard (rate reset)
  def loan_card_rate_reset
    loan = build_loan
    inst = loan.instalments.find { |i| i.paid? && i.number == 5 }
    inst&.instance_variable_set(:@previous_amount_cents, 76_800)
    render Campbooks::Money::LoanCard.new(loan: loan)
  end

  # Loan with a missed instalment (amber tick).
  # @label LoanCard (missed instalment)
  def loan_card_missed
    loan = build_loan
    missed = loan.instalments.find { |i| i.expected? && i.number == 10 }
    missed&.instance_variable_set(:@status, "missed")
    render Campbooks::Money::LoanCard.new(loan: loan)
  end

  # -------------------------------------------------------------------
  # LoanSuggestionRow
  # -------------------------------------------------------------------

  # Needs-you row when Scout spots a probable loan.
  # @label LoanSuggestionRow
  def loan_suggestion_row
    suggestion = build_suggestion
    render Campbooks::Money::LoanSuggestionRow.new(suggestion: suggestion)
  end

  # -------------------------------------------------------------------
  # LoanForm
  # -------------------------------------------------------------------

  # New loan form — blank (no suggestion or existing loan).
  # @label LoanForm (blank)
  def loan_form_blank
    render Campbooks::Money::LoanForm.new
  end

  # New loan form prefilled from a Spotter suggestion.
  # @label LoanForm (from suggestion)
  def loan_form_from_suggestion
    render Campbooks::Money::LoanForm.new(suggestion: build_suggestion)
  end

  # Edit form prefilled from an existing loan.
  # @label LoanForm (edit existing)
  def loan_form_edit
    render Campbooks::Money::LoanForm.new(loan: build_loan)
  end

  # -------------------------------------------------------------------
  # LoanAlertRow
  # -------------------------------------------------------------------

  # Missed-instalment alert (amber).
  # @label LoanAlertRow (missed)
  def loan_alert_row_missed
    loan = build_loan
    instalment = loan.instalments.find(&:expected?)
    render Campbooks::Money::LoanAlertRow.new(loan: loan, instalment: instalment, kind: :missed)
  end

  # Changed-amount alert (rate reset).
  # @label LoanAlertRow (changed)
  def loan_alert_row_changed
    loan = build_loan
    instalment = loan.instalments.find(&:paid?)
    render Campbooks::Money::LoanAlertRow.new(loan: loan, instalment: instalment, kind: :changed)
  end

  # -------------------------------------------------------------------
  # LoanStat
  # -------------------------------------------------------------------

  # Compact strip stat for the loan.
  # @label LoanStat
  def loan_stat
    render Campbooks::Money::LoanStat.new(loan: build_loan)
  end

  private

  # ------------------------------------------------------------------
  # Fake Loan + LoanInstalment stubs (no DB needed)
  # ------------------------------------------------------------------

  FakeInstalment = Struct.new(
    :id, :loan_id, :number, :expected_on, :amount_cents, :previous_amount_cents,
    :bank_transaction_id, :bank_transaction, :status, :note, :created_at, :updated_at,
    keyword_init: true
  ) do
    def paid?        = status.to_s == "paid"
    def unverified?  = status.to_s == "unverified"
    def missed?      = status.to_s == "missed"
    def expected?    = status.to_s == "expected"
    def status_sym   = status.to_sym
    def ordered_scope = self
  end

  FakeLoan = Struct.new(
    :id, :workspace_id, :created_by_id, :lender, :source_counterparty,
    :principal_cents, :currency, :instalment_cents, :first_instalment_on,
    :term_months, :rate_note, :notes, :status, :change_acknowledged_at,
    :created_at, :updated_at, :instalments,
    keyword_init: true
  ) do
    def active? = status.to_s == "active"

    def paid_instalments   = instalments.select { |i| i.paid? || i.unverified? }
    def paid_count         = paid_instalments.count
    def paid_cents         = paid_instalments.sum(&:amount_cents)
    def remaining_instalments = instalments.select { |i| i.expected? || i.missed? }
    def remaining_count    = remaining_instalments.count
    def remaining_cents    = remaining_count * instalment_cents
    def next_expected      = instalments.select(&:expected?).min_by(&:expected_on)
    def last_seen
      instalments.select(&:paid?).select { |i| i.bank_transaction.present? }.max_by(&:expected_on)
    end
    def ends_on            = instalments.map(&:expected_on).max
    def amount_changed_at  = nil
    def progress_pct       = paid_count.to_f / term_months * 100
    def effective_counterparty = source_counterparty.presence || lender
  end

  def build_loan
    today = Date.current
    first = Date.new(2023, 8, 5)
    term = 60

    instalments = (1..term).map do |n|
      expected_on = first >> (n - 1)
      status = if n <= 5
                 "unverified"
               elsif n <= 18
                 "paid"
               elsif n == 19
                 "expected"
               else
                 "expected"
               end
      FakeInstalment.new(
        id: "inst-#{n}", loan_id: "loan-1", number: n,
        expected_on: expected_on,
        amount_cents: 78_000, previous_amount_cents: nil,
        bank_transaction_id: (status == "paid" ? "tx-#{n}" : nil),
        bank_transaction: nil, status: status,
        note: nil, created_at: today, updated_at: today
      )
    end

    FakeLoan.new(
      id: "loan-1", workspace_id: "ws-1", created_by_id: "user-1",
      lender: "Millennium BCP",
      source_counterparty: "MILLENNIUM BCP",
      principal_cents: 4_680_000,
      currency: "EUR",
      instalment_cents: 78_000,
      first_instalment_on: first,
      term_months: term,
      rate_note: "Euribor 12M + 1.5%",
      notes: nil,
      status: "active",
      change_acknowledged_at: nil,
      created_at: today - 300, updated_at: today,
      instalments: instalments
    )
  end

  def build_suggestion
    today = Date.current
    Loans::Spotter::Suggestion.new(
      lender_guess: "Millennium BCP",
      source_counterparty: "MILLENNIUM BCP",
      instalment_cents: 78_000,
      currency: "EUR",
      first_seen_on: today - 540,
      last_seen_on: today - 5,
      count: 18,
      day_of_month: 5,
      previous_instalment_cents: nil,
      sample_transaction_ids: [],
      key: "millennium bcp|78000"
    )
  end
end
