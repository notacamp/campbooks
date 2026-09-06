# frozen_string_literal: true

class Money
  # Pure data object the Scout note and the Strip read from.
  # Built from an Evidence and Ledger that Money::Page has already constructed.
  #
  #   read = Money::Read.for(workspace, user, today:, evidence:, ledger:, groups:, loans:, suggestions:)
  #   read.lines_total        # integer
  #   read.explained_pct      # 0-100
  #   read.any_statements?    # boolean
  #   read.loan_state         # :none | :suggestion | :tracked | :on_schedule | :seen | :missed | :changed
  class Read
    ON_TIME_GRACE = 5 # days after expected_on an instalment still reads "on time"

    def self.for(workspace, user, today: Date.current, evidence: nil, ledger: nil, groups: nil,
                 loans: nil, suggestions: nil)
      ev  = evidence || Money::Evidence.for(workspace)
      led = ledger  || Money::Ledger.for(workspace, user, today: today, evidence: ev)
      new(workspace, user, today, ev, led, groups, loans, suggestions)
    end

    def initialize(workspace, user, today, evidence, ledger, prebuilt_groups = nil, loans = nil, suggestions = nil)
      @workspace       = workspace
      @user            = user
      @today           = today
      @evidence        = evidence
      @ledger          = ledger
      @prebuilt_groups = prebuilt_groups
      @loans           = loans
      @suggestions     = suggestions
    end

    # The newest ready statement, or nil.
    def statement
      @evidence.statements.first
    end

    def statement_label
      statement ? @evidence.label_for(statement) : nil
    end

    def bank_name
      statement&.bank_name
    end

    def any_statements? = @evidence.any?

    # Transaction counts for the newest statement.
    def lines_total
      @lines_total ||= statement ? statement.bank_transactions.count : 0
    end

    def lines_explained
      @lines_explained ||=
        if statement
          status_counts.values_at(*BankTransaction::RESOLVED_STATUSES.map(&:to_s)).sum { |v| v.to_i }
        else
          0
        end
    end

    def review_count
      @review_count ||= status_counts.fetch("suggested", 0)
    end

    def needs_invoice_count
      @needs_invoice_count ||=
        if statement
          statement.bank_transactions.where(status: :unmatched).where("amount_cents < 0").count
        else
          0
        end
    end

    def needs_invoice_cents
      @needs_invoice_cents ||=
        if statement
          statement.bank_transactions.where(status: :unmatched)
                   .where("amount_cents < 0")
                   .sum("ABS(amount_cents)")
        else
          0
        end
    end

    def requested_count
      @requested_count ||= status_counts.fetch("requested", 0)
    end

    def partial_count
      @partial_count ||= groups.count { |g| g.kind == :partial }
    end

    def nif_count
      @nif_count ||= statement&.nif_exception_count(@workspace.company_nif.presence) || 0
    end

    def missing_count
      @missing_count ||= @ledger.missing.size
    end

    def missing_cents
      @missing_cents ||= missing_cents_by_currency.fetch(primary_currency, 0)
    end

    def missing_cents_by_currency
      @missing_cents_by_currency ||= @ledger.missing_cents_by_currency
    end

    def primary_currency
      @primary_currency ||=
        @workspace.try(:default_currency).presence ||
        @workspace.try(:currency).presence ||
        ::Money.default_currency.iso_code
    end

    def explained_pct
      return 0 if lines_total.zero?

      (lines_explained * 100.0 / lines_total).round
    end

    # ── The loan ─────────────────────────────────────────────────────────────

    def loans
      @loans ||= @workspace.loans.active_loans.order(:created_at).to_a
    end

    def loan
      loans.first
    end

    def loan_suggestion
      suggestions.first
    end

    def suggestions
      @suggestions ||= loans.empty? ? Loans::Spotter.new(@workspace).call : []
    end

    # What Scout can say about the loan, most pressing first.
    def loan_state
      @loan_state ||=
        if loan.nil?
          loan_suggestion ? :suggestion : :none
        elsif loan.unacknowledged_change
          :changed
        elsif loan.missed_instalments.exists?
          :missed
        elsif loan.last_seen.nil?
          :tracked
        elsif loan_on_time?
          :on_schedule
        else
          :seen
        end
    end

    def loan_last_seen
      loan&.last_seen
    end

    def loan_on_time?
      seen = loan_last_seen
      txn  = seen&.bank_transaction
      seen && txn && txn.booked_on <= seen.expected_on + ON_TIME_GRACE.days
    end

    private

    def groups
      @groups ||= @prebuilt_groups || (statement ? Reconciliations::Groups.new(statement).call : [])
    end

    def status_counts
      @status_counts ||= statement ? statement.bank_transactions.group(:status).count : {}
    end
  end
end
