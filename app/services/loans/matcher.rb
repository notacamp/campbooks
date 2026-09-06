# frozen_string_literal: true

module Loans
  # Links bank lines in a Reconciliation to the workspace's active loans.
  # Runs as a pre-pass in Reconciliations::MatchJob BEFORE the invoice matcher, so
  # loan lines are claimed first and the invoice matcher only ever sees :unmatched
  # lines.
  #
  # Rules:
  #  - Candidates: :unmatched debits, plus lines set aside with the reason "loan",
  #    in the loan's currency, not yet linked to an instalment.
  #  - Evidence: LOAN_KEYWORDS in description/counterparty OR lender-token overlap.
  #  - Amount: within 3% of the loan's current instalment or of any expected one.
  #  - Instalment: the earliest expected/missed one within +/-12 days of booked_on,
  #    else the earliest one due by booked_on + 12 days (catch-up).
  #  - Amount changes (rate resets) are derived afterwards by Loans::Status from the
  #    chronological sequence of paid instalments, so backfilling older statements
  #    after newer ones never rewrites the current instalment the wrong way.
  #  - Idempotent: linked lines are skipped.
  class Matcher
    include ActionView::RecordIdentifier

    LOAN_KEYWORDS = /\b(prest\w*|empr[eé]stimo|emprest\w*|cr[eé]dito|amortiza\w*|loan|financiamento|leasing|pr[eê]t|cuota|hipoteca)\b/i
    LEGAL_NOISE   = %w[sa s.a. lda ltd ltda plc bank banco banque].freeze
    AMOUNT_TOLERANCE = 0.03  # 3%
    DATE_WINDOW      = 12    # days

    def initialize(reconciliation)
      @reconciliation = reconciliation
      @workspace      = reconciliation.workspace
    end

    def call
      loans = @workspace.loans.active_loans.includes(:instalments).to_a
      return if loans.empty?

      candidates = candidate_transactions
      loans.each { |loan| process_loan(loan, candidates) }
      loans.each { |loan| Loans::Status.refresh!(loan) }
      broadcast_summary_bar
    end

    # Link one line to one loan by hand (the hunt panel's "Loan instalment" reason).
    # Returns the instalment, or nil when nothing on the schedule fits the date.
    def link_line!(txn, loan)
      return nil unless txn.debit? && txn.currency == loan.currency

      instalment = find_instalment(txn, loan)
      return nil unless instalment

      link!(txn, instalment)
      Loans::Status.refresh!(loan)
      instalment
    end

    private

    def candidate_transactions
      scope = @reconciliation.bank_transactions
      scope.where(status: :unmatched)
           .or(scope.where(status: :excluded, exclusion_reason: "loan"))
           .where("amount_cents < 0")
           .includes(:loan_instalment)
           .to_a
           .reject { |txn| txn.loan_instalment.present? }
    end

    def process_loan(loan, candidates)
      candidates.each do |txn|
        next if txn.explained? # claimed by another loan earlier in this run
        next unless txn.currency == loan.currency
        next unless evidence_for_loan?(txn, loan) && amount_matches?(txn, loan)

        instalment = find_instalment(txn, loan)
        next unless instalment

        link!(txn, instalment)
      end
    end

    def evidence_for_loan?(txn, loan)
      text = "#{txn.description} #{txn.counterparty}".downcase
      return true if LOAN_KEYWORDS.match?(text)

      lender_tokens(loan).any? { |token| text.include?(token) }
    end

    def lender_tokens(loan)
      loan.effective_counterparty.to_s.split(/\W+/).map(&:downcase)
          .reject { |t| t.length < 3 || LEGAL_NOISE.include?(t) }
    end

    def amount_matches?(txn, loan)
      abs = txn.amount_cents.abs.to_f
      return true if close?(abs, loan.instalment_cents)

      loan.instalments.any? { |ins| ins.expected? && close?(abs, ins.amount_cents) }
    end

    def close?(abs, scheduled)
      scheduled.to_i.positive? && (abs - scheduled).abs / scheduled.to_f <= AMOUNT_TOLERANCE
    end

    def find_instalment(txn, loan)
      date     = txn.booked_on
      eligible = loan.instalments
                     .select { |ins| (ins.expected? || ins.missed?) && ins.bank_transaction_id.nil? }
                     .sort_by(&:expected_on)

      within = eligible.find { |ins| (ins.expected_on - date).abs <= DATE_WINDOW }
      within || eligible.find { |ins| ins.expected_on <= date + DATE_WINDOW.days }
    end

    def link!(txn, instalment)
      instalment.update!(bank_transaction: txn, status: :paid, amount_cents: txn.amount_cents.abs)
      txn.update!(status: :explained, exclusion_reason: nil)
      broadcast_row(txn)
    end

    # Keep an open workbench in sync (same partials the workbench actions replace).
    def broadcast_row(txn)
      locale = @reconciliation.created_by&.locale.presence || I18n.default_locale
      I18n.with_locale(locale) do
        stream = "reconciliation_#{@reconciliation.id}"
        Turbo::StreamsChannel.broadcast_replace_to(
          stream, target: dom_id(txn),
          html: ApplicationController.render(partial: "bank_transactions/row",
                                             locals: { transaction: txn, reconciliation: @reconciliation }, layout: false)
        )
        Turbo::StreamsChannel.broadcast_replace_to(
          stream, target: dom_id(txn, :card),
          html: ApplicationController.render(partial: "bank_transactions/card",
                                             locals: { transaction: txn, reconciliation: @reconciliation }, layout: false)
        )
      end
    rescue => e
      Rails.logger.warn("[Loans::Matcher] broadcast failed: #{e.class}: #{e.message}")
    end

    def broadcast_summary_bar
      counts = @reconciliation.bank_transactions.group(:status).count
      html   = ApplicationController.render(
        partial: "reconciliations/summary_bar",
        locals:  { reconciliation: @reconciliation, status_counts: counts,
                   nif_exception_count: @reconciliation.nif_exception_count(@workspace.company_nif.presence) },
        layout:  false
      )
      Turbo::StreamsChannel.broadcast_replace_to("reconciliation_#{@reconciliation.id}",
                                                 target: "reconciliation_summary_bar", html: html)
    rescue => e
      Rails.logger.warn("[Loans::Matcher] summary broadcast failed: #{e.class}: #{e.message}")
    end
  end
end
