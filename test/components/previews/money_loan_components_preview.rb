# frozen_string_literal: true

# Lookbook previews for the Money loan components: LoanCard, LoanSuggestionRow,
# LoanForm, LoanAlertRow, LoanStat. In-memory stubs, no database: the stubs answer
# exactly the Loan/LoanInstalment helpers the components read.
class MoneyLoanComponentsPreview < ViewComponent::Preview
  # 18 of 60 paid, on schedule, the next one expected.
  # @label LoanCard (on schedule)
  def loan_card_active
    render Campbooks::Money::LoanCard.new(loan: build_loan)
  end

  # The instalment stepped from 768 to 780 at instalment 16 (a rate reset).
  # @label LoanCard (rate reset)
  def loan_card_rate_reset
    loan = build_loan
    loan.instalments.select { |i| i.number < 16 }.each { |i| i.amount_cents = 76_800 }
    loan.instalments.find { |i| i.number == 16 }.previous_amount_cents = 76_800
    render Campbooks::Money::LoanCard.new(loan: loan)
  end

  # Instalment 19 should have been on a reconciled statement and wasn't.
  # @label LoanCard (missed instalment)
  def loan_card_missed
    loan = build_loan
    loan.instalments.find { |i| i.number == 19 }.status = "missed"
    render Campbooks::Money::LoanCard.new(loan: loan)
  end

  # Scout's guess, with the prefilled form under "Track the loan".
  # @label LoanSuggestionRow
  def loan_suggestion_row
    render Campbooks::Money::LoanSuggestionRow.new(suggestion: build_suggestion)
  end

  # @label LoanForm (blank)
  def loan_form_blank
    render Campbooks::Money::LoanForm.new
  end

  # @label LoanForm (from suggestion)
  def loan_form_from_suggestion
    render Campbooks::Money::LoanForm.new(suggestion: build_suggestion)
  end

  # @label LoanForm (edit existing)
  def loan_form_edit
    render Campbooks::Money::LoanForm.new(loan: build_loan)
  end

  # @label LoanAlertRow (missed)
  def loan_alert_row_missed
    loan = build_loan
    instalment = loan.instalments.find { |i| i.number == 19 }
    instalment.status = "missed"
    render Campbooks::Money::LoanAlertRow.new(loan: loan, instalment: instalment, kind: :missed)
  end

  # @label LoanAlertRow (changed)
  def loan_alert_row_changed
    loan = build_loan
    instalment = loan.instalments.find { |i| i.number == 16 }
    instalment.previous_amount_cents = 76_800
    render Campbooks::Money::LoanAlertRow.new(loan: loan, instalment: instalment, kind: :changed)
  end

  # @label LoanStat
  def loan_stat
    render Campbooks::Money::LoanStat.new(loan: build_loan)
  end

  private

  FakeTransaction = Struct.new(:id, :description, :booked_on, :reconciliation, keyword_init: true)

  FakeInstalment = Struct.new(
    :id, :number, :expected_on, :amount_cents, :previous_amount_cents, :bank_transaction_id, :bank_transaction, :status,
    keyword_init: true
  ) do
    def paid?       = status.to_s == "paid"
    def unverified? = status.to_s == "unverified"
    def missed?     = status.to_s == "missed"
    def expected?   = status.to_s == "expected"
  end

  FakeWorkspace = Struct.new(:reconciliations, keyword_init: true)

  FakeLoan = Struct.new(
    :id, :lender, :source_counterparty, :principal_cents, :currency, :instalment_cents, :first_instalment_on,
    :term_months, :rate_note, :status, :change_acknowledged_at, :instalments, :workspace,
    keyword_init: true
  ) do
    def paid_instalments      = instalments.select { |i| i.paid? || i.unverified? }
    def paid_count            = paid_instalments.size
    def paid_cents            = paid_instalments.sum(&:amount_cents)
    def remaining_instalments = instalments.select { |i| i.expected? || i.missed? }
    def remaining_count       = remaining_instalments.size
    def remaining_cents       = remaining_count * instalment_cents
    def missed_instalments    = instalments.select(&:missed?).sort_by(&:expected_on)
    def next_expected(today = Date.current) = instalments.select { |i| i.expected? && i.expected_on >= today }.min_by(&:expected_on)
    def awaiting_statement(today = Date.current) = instalments.select { |i| i.expected? && i.expected_on < today }.sort_by(&:expected_on)
    def last_seen             = instalments.select { |i| i.paid? && i.bank_transaction }.max_by(&:expected_on)
    def recent_paid(limit = 3) = instalments.select { |i| i.paid? && i.bank_transaction }.sort_by(&:expected_on).reverse.first(limit)
    def ends_on               = instalments.map(&:expected_on).max
    def amount_changed_at     = instalments.select { |i| i.paid? && i.previous_amount_cents }.max_by(&:expected_on)
    def unacknowledged_change = amount_changed_at
    def progress_pct          = (paid_count.to_f / term_months * 100).round
    def effective_counterparty = source_counterparty.presence || lender
    def to_param              = id
  end

  # 60 instalments from Aug 2023: 1-5 before the first statement, 6-18 found on
  # statements, 19 expected (its statement isn't in yet), the rest to come.
  def build_loan
    first = Date.new(2023, 8, 5)
    instalments = (1..60).map do |n|
      status = n <= 5 ? "unverified" : (n <= 18 ? "paid" : "expected")
      txn = if status == "paid"
        FakeTransaction.new(id: "txn-#{n}", description: "PREST EMPRESTIMO #{n}/60", booked_on: first >> (n - 1), reconciliation: nil)
      end
      FakeInstalment.new(id: "ins-#{n}", number: n, expected_on: first >> (n - 1), amount_cents: 78_000,
                         previous_amount_cents: nil, bank_transaction_id: txn&.id, bank_transaction: txn, status: status)
    end

    FakeLoan.new(
      id: "loan-1", lender: "Millennium BCP", source_counterparty: "MILLENNIUM BCP",
      principal_cents: 4_680_000, currency: "EUR", instalment_cents: 78_000, first_instalment_on: first,
      term_months: 60, rate_note: "Euribor 12M + 1.5%", status: "active", change_acknowledged_at: nil,
      instalments: instalments, workspace: FakeWorkspace.new(reconciliations: Reconciliation.none)
    )
  end

  def build_suggestion
    Loans::Spotter::Suggestion.new(
      lender_guess: "Millennium BCP", source_counterparty: "MILLENNIUM BCP",
      instalment_cents: 78_000, currency: "EUR",
      first_seen_on: Date.new(2025, 3, 5), last_seen_on: Date.new(2026, 8, 5),
      count: 18, day_of_month: 5, previous_instalment_cents: 76_800,
      sample_transaction_ids: [], key: "millennium bcp|78000"
    )
  end
end
