# frozen_string_literal: true

class Money
  # One builder for the entire Money surface. MoneyController, LoansController and
  # Reconciliations::BankTransactionsController (when surface=money) all render
  # money/_content from this object, keeping derived state consistent after every
  # action.
  #
  #   page = Money::Page.for(workspace, user, today:, statement_id:)
  #   page.evidence           # Money::Evidence
  #   page.ledger             # Money::Ledger
  #   page.read               # Money::Read (always about the NEWEST statement)
  #   page.needs_you          # Array<Money::NeedsYouItem> (newest statement + the loan)
  #   page.loans              # active Loans, oldest first
  #   page.loan_suggestions   # Loans::Spotter suggestions (only when nothing is tracked yet)
  #   page.selected_statement # Reconciliation or nil (the Statements tab)
  #   page.statement_counts   # { reconciliation_id => [resolved, total] } for the tabs
  class Page
    NEEDS_YOU_CAP = 8

    def self.for(workspace, user, today: Date.current, statement_id: nil)
      new(workspace, user, today: today, statement_id: statement_id)
    end

    attr_reader :evidence, :ledger, :read, :company_nif

    def initialize(workspace, user, today:, statement_id: nil)
      @workspace    = workspace
      @user         = user
      @today        = today
      @statement_id = statement_id
      @company_nif  = workspace.company_nif.presence
      @groups_cache = {}

      @evidence = Money::Evidence.for(workspace, today: today)
      @ledger   = Money::Ledger.for(workspace, user, today: today, evidence: @evidence)
      @read     = Money::Read.for(workspace, user,
                                  today:       today,
                                  evidence:    @evidence,
                                  ledger:      @ledger,
                                  groups:      newest_groups,
                                  loans:       loans,
                                  suggestions: loan_suggestions)
    end

    # The statement Scout reads and Needs-you is lifted from.
    def newest_statement
      @evidence.statements.first
    end

    # Every reconciled statement on the month timeline (the pills), newest first.
    def statements
      @statements ||= months.flat_map(&:statements).uniq
    end

    # The tab the user is looking at; falls back to the newest statement.
    def selected_statement
      @selected_statement ||= begin
        chosen = @workspace.reconciliations.ready.find_by(id: @statement_id) if @statement_id.present?
        chosen || newest_statement
      end
    end

    def selected_groups
      groups_for(selected_statement)
    end

    def newest_groups
      groups_for(newest_statement)
    end

    # One pair of grouped counts for every tab (no per-tab queries).
    def statement_counts
      @statement_counts ||= begin
        ids      = (statements + [ selected_statement ].compact).map(&:id).uniq
        totals   = BankTransaction.where(reconciliation_id: ids).group(:reconciliation_id).count
        resolved = BankTransaction.where(reconciliation_id: ids, status: BankTransaction::RESOLVED_STATUSES)
                                  .group(:reconciliation_id).count
        ids.index_with { |id| [ resolved.fetch(id, 0), totals.fetch(id, 0) ] }
      end
    end

    # ── The loan ─────────────────────────────────────────────────────────────
    def loans
      @loans ||= @workspace.loans.active_loans.order(:created_at).to_a
    end

    # Scout's guess at an untracked loan. Only worth raising while nothing is
    # tracked yet; a second loan is added by hand.
    def loan_suggestions
      @loan_suggestions ||= loans.empty? ? Loans::Spotter.new(@workspace).call : []
    end

    # The month timeline (newest first): the month to reconcile, then every month
    # back to the first statement, each with the statements that cover it.
    def months
      @evidence.months
    end

    def pending_statement_documents
      @read.pending_statement_documents
    end

    def needs_you
      all_needs_you.first(NEEDS_YOU_CAP)
    end

    def needs_you_overflow
      [ all_needs_you.size - NEEDS_YOU_CAP, 0 ].max
    end

    def missing
      @ledger.missing
    end

    private

    def groups_for(statement)
      return [] unless statement

      @groups_cache[statement.id] ||= Reconciliations::Groups.new(statement).call
    end

    def all_needs_you
      @all_needs_you ||= failed_statement_needs_you + month_needs_you + statement_needs_you + loan_needs_you
    end

    # Statements that couldn't be read (the AI provider was busy, or the file
    # is bad): one row each with "Try again", so recovery is a click, not
    # delete-and-add-again.
    def failed_statement_needs_you
      @workspace.reconciliations.where(status: :failed).includes(:statement_document).order(created_at: :desc).map do |recon|
        NeedsYouItem.new(kind: :statement_failed, payload: { reconciliation: recon })
      end
    end

    # The month to reconcile, before the lines: statements Scout already holds
    # that nobody reconciled, else the month whose statement isn't in yet. A
    # workspace with no statements at all gets the empty state instead.
    def month_needs_you
      if @read.pending_statement_count.positive?
        [ NeedsYouItem.new(kind: :reconcile_statements,
                           payload: { documents: @read.pending_statement_documents, count: @read.pending_statement_count }) ]
      elsif @read.any_statements? && !@read.focus_reconciled?
        [ NeedsYouItem.new(kind: :add_statement, payload: { month: @read.focus_month, label: @read.focus_label }) ]
      else
        []
      end
    end

    def statement_needs_you # rubocop:disable Metrics/MethodLength
      stmt = newest_statement
      return [] unless stmt

      txns = stmt.bank_transactions
                 .includes(transaction_matches: :document)
                 .order(:position)
                 .to_a

      items = []

      # 1. :no_invoice — each unmatched debit
      txns.select { |t| t.unmatched? && t.debit? }.each do |txn|
        items << NeedsYouItem.new(
          kind:        :no_invoice,
          transaction: txn,
          title:       I18n.t("money.needs_you.no_invoice.title"),
          meta:        build_no_invoice_meta(txn),
          actions:     [ :resolve ]
        )
      end

      # 2. :review — each suggested transaction with its best match
      txns.select(&:suggested?).each do |txn|
        match = txn.transaction_matches.select(&:suggested?).max_by(&:confidence)
        next unless match

        nif_flag = @company_nif.present? && match.document&.nif_status(@company_nif)&.in?(%i[missing mismatch])
        items << NeedsYouItem.new(
          kind:        :review,
          transaction: txn,
          match:       match,
          title:       I18n.t("money.needs_you.review.title"),
          meta:        build_review_meta(txn, match, nif_flag),
          actions:     [ :change, :confirm ]
        )
      end

      # 3. :partial — each partial group of the newest statement
      newest_groups.select { |g| g.kind == :partial }.each do |group|
        txn    = group.bank_transactions.first
        doc    = group.documents.first
        vendor = doc&.entity_display_name.presence || txn&.counterparty.presence || txn&.description.to_s
        items << NeedsYouItem.new(
          kind:        :partial,
          transaction: txn,
          group:       group,
          title:       I18n.t("money.needs_you.partial.title", vendor: vendor),
          meta:        build_partial_meta(group),
          actions:     [ :open_statement ]
        )
      end

      # 4. :nif — matched lines whose top document is NIF-flagged
      if @company_nif.present?
        txns.select(&:matched?).each do |txn|
          next unless txn.nif_flagged?(@company_nif)

          items << NeedsYouItem.new(
            kind:        :nif,
            transaction: txn,
            title:       I18n.t("money.needs_you.nif.title"),
            meta:        [ txn.counterparty.presence || I18n.t("money.needs_you.no_name"), format_amount(txn) ],
            actions:     [ :request_invoice ]
          )
        end
      end

      items
    end

    # 5. the loan: an instalment missing from a reconciled statement, an amount
    #    change nobody has waved through, or Scout's untracked-loan guess.
    def loan_needs_you
      items = []

      loans.each do |loan|
        loan.missed_instalments.each do |instalment|
          items << NeedsYouItem.new(kind: :loan_missed, payload: { loan: loan, instalment: instalment })
        end
        if (changed = loan.unacknowledged_change)
          items << NeedsYouItem.new(kind: :loan_changed, payload: { loan: loan, instalment: changed })
        end
      end

      loan_suggestions.each do |suggestion|
        items << NeedsYouItem.new(kind: :loan_suggestion, payload: suggestion)
      end

      items
    end

    def build_no_invoice_meta(txn)
      name = txn.counterparty.presence || I18n.t("money.needs_you.no_name")
      [ name, format_amount(txn), I18n.l(txn.booked_on, format: :date) ]
    end

    def build_review_meta(txn, match, nif_flag)
      parts = [ txn.counterparty.presence || match.document&.entity_display_name.to_s, format_amount(txn) ]
      inv = match.document&.invoice_number
      parts << I18n.t("money.what.invoice", number: inv) if inv.present?
      parts << "#{(match.confidence.to_f * 100).round}% #{I18n.t('money.needs_you.review.likely')}"
      parts << { nif: true } if nif_flag
      parts
    end

    def build_partial_meta(group)
      currency    = group.bank_transactions.first&.currency || "EUR"
      paid        = ::Money.new(group.allocated_cents.to_i, currency)
      total       = ::Money.new(group.invoice_total_cents.to_i, currency)
      outstanding = ::Money.new(group.outstanding_cents.to_i, currency)
      [ I18n.t("money.needs_you.partial.meta", paid: paid.format, total: total.format, outstanding: outstanding.format) ]
    end

    def format_amount(txn)
      sign = txn.debit? ? "−" : "+"
      "#{sign}#{::Money.new(txn.amount_cents.abs, txn.currency.presence || 'EUR').format}"
    end
  end
end
