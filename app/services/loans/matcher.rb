# frozen_string_literal: true

module Loans
  # Links bank lines in a Reconciliation to known active loans.
  # Runs as a pre-pass in Reconciliations::MatchJob BEFORE the invoice matcher, so
  # loan lines are claimed first; the invoice matcher only ever sees :unmatched lines.
  #
  # Rules:
  #  - Candidates: :unmatched debits (and :excluded with exclusion_reason == "loan"),
  #    same currency as the loan.
  #  - Evidence: LOAN_KEYWORDS in description/counterparty OR lender-token overlap.
  #  - Amount: within 3% of loan.instalment_cents OR any expected instalment's amount.
  #  - Instalment: earliest expected/missed within +/-12 days of booked_on; else catch-up.
  #  - Rate reset: when paid amount differs from loan.instalment_cents by >= 0.5%.
  #  - Idempotent: already-explained lines are skipped.
  class Matcher
    include ActionView::RecordIdentifier

    LOAN_KEYWORDS = /\b(prest\w*|empr[eé]stimo|emprest\w*|cr[eé]dito|amortiza\w*|loan|financiamento|leasing|pr[eê]t|cuota|hipoteca)\b/i
    LEGAL_NOISE   = %w[sa s.a. lda ltd ltda plc bank banco banque].freeze
    AMOUNT_TOLERANCE = 0.03  # 3%
    RATE_RESET_THRESHOLD = 0.005  # 0.5%
    DATE_WINDOW = 12  # days

    def initialize(reconciliation)
      @reconciliation = reconciliation
      @workspace      = reconciliation.workspace
    end

    def call
      loans = @workspace.loans.active_loans.includes(:instalments)
      return if loans.empty?

      candidates = candidate_transactions

      loans.each do |loan|
        process_loan(loan, candidates)
      end

      Loans::Status.refresh!(@workspace.loans.active_loans.first) if @workspace.loans.active_loans.any?
    end

    private

    def candidate_transactions
      @reconciliation.bank_transactions
        .where(status: [ BankTransaction.statuses[:unmatched],
                         BankTransaction.statuses[:excluded] ])
        .where("amount_cents < 0")  # debits only
        .includes(:loan_instalment)
        .to_a
        .reject { |txn| txn.loan_instalment.present? }  # skip already linked
    end

    def process_loan(loan, candidates)
      # Filter to currency-matching candidates
      loan_candidates = candidates.select { |txn| txn.currency == loan.currency }
      loan_candidates.each do |txn|
        next unless evidence_for_loan?(txn, loan)
        next unless amount_matches?(txn, loan)

        instalment = find_instalment(txn, loan)
        next unless instalment

        link!(txn, instalment, loan)
      end
    end

    def evidence_for_loan?(txn, loan)
      text = "#{txn.description} #{txn.counterparty}".downcase

      # Keyword match
      return true if LOAN_KEYWORDS.match?(text)

      # Lender token match
      lender_tokens(loan).any? do |token|
        text.include?(token.downcase)
      end
    end

    def lender_tokens(loan)
      name = loan.effective_counterparty.to_s
      tokens = name.split(/\W+/).map(&:downcase).reject do |t|
        t.length < 3 || LEGAL_NOISE.include?(t)
      end
      tokens
    end

    def amount_matches?(txn, loan)
      abs = txn.amount_cents.abs.to_f

      # Within 3% of current instalment
      return true if (abs - loan.instalment_cents).abs / loan.instalment_cents.to_f <= AMOUNT_TOLERANCE

      # Within 3% of any expected instalment's scheduled amount
      loan.instalments.where(status: :expected).pluck(:amount_cents).any? do |scheduled|
        next if scheduled.zero?

        (abs - scheduled).abs / scheduled.to_f <= AMOUNT_TOLERANCE
      end
    end

    def find_instalment(txn, loan)
      date = txn.booked_on

      # Eligible: expected or missed, not already linked
      eligible = loan.instalments
                     .where(status: %i[expected missed])
                     .where(bank_transaction_id: nil)
                     .order(:expected_on)
                     .to_a

      # 1. Within +/-12 days of booked_on
      within_window = eligible.select do |ins|
        (ins.expected_on - date).abs <= DATE_WINDOW
      end
      return within_window.first if within_window.any?

      # 2. Catch-up: expected_on <= booked_on + 12 days
      catch_up = eligible.select do |ins|
        ins.expected_on <= date + DATE_WINDOW.days
      end
      catch_up.first
    end

    def link!(txn, instalment, loan)
      paid_cents = txn.amount_cents.abs
      scheduled  = instalment.amount_cents

      previous = if (paid_cents - scheduled).abs.to_f / scheduled > RATE_RESET_THRESHOLD
                   scheduled
                 end

      # Rate reset: update current instalment and all later expected rows
      if previous.present? && (paid_cents - loan.instalment_cents).abs.to_f / loan.instalment_cents > RATE_RESET_THRESHOLD
        loan.update_columns(instalment_cents: paid_cents)
        loan.instalments.where(status: :expected).update_all(amount_cents: paid_cents)
      end

      instalment.update!(
        bank_transaction: txn,
        status:           :paid,
        amount_cents:     paid_cents,
        previous_amount_cents: previous
      )

      txn.update!(status: :explained, exclusion_reason: nil)

      broadcast_row(txn)
    end

    def broadcast_row(txn)
      locale = @reconciliation.created_by&.locale.presence || I18n.default_locale

      I18n.with_locale(locale) do
        html = ApplicationController.render(
          partial: "reconciliations/bank_transaction_row",
          locals:  { bank_transaction: txn, reconciliation: @reconciliation },
          layout:  false
        )

        Turbo::StreamsChannel.broadcast_replace_to(
          "reconciliation_#{@reconciliation.id}",
          target: helpers.dom_id(txn),
          html:   html
        )
      end
    rescue => e
      Rails.logger.warn("[Loans::Matcher] broadcast failed: #{e.class}: #{e.message}")
    end

    def helpers
      @helpers ||= ActionController::Base.helpers
    end
  end
end
