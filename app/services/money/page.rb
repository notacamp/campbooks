# frozen_string_literal: true

class Money
  # One builder for the entire Money surface. Both MoneyController and
  # Reconciliations::BankTransactionsController (when surface=money) render
  # money/_content from this object, keeping derived state consistent after
  # every action.
  #
  #   page = Money::Page.for(workspace, user, today:, statement_id:)
  #   page.evidence           # Money::Evidence
  #   page.ledger             # Money::Ledger
  #   page.read               # Money::Read (always about the NEWEST statement)
  #   page.needs_you          # Array<Money::NeedsYouItem> (newest statement)
  #   page.selected_statement # Reconciliation or nil (the Statements tab)
  #   page.statement_counts   # { reconciliation_id => [resolved, total] } for the tabs
  class Page
    NEEDS_YOU_CAP = 8
    TAB_LIMIT     = 6

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

      @evidence = Money::Evidence.for(workspace)
      @ledger   = Money::Ledger.for(workspace, user, today: today, evidence: @evidence)
      @read     = Money::Read.for(workspace, user,
                                  today:    today,
                                  evidence: @evidence,
                                  ledger:   @ledger,
                                  groups:   newest_groups)
    end

    # The statement Scout reads and Needs-you is lifted from.
    def newest_statement
      @evidence.statements.first
    end

    def statements
      @statements ||= @evidence.statements.first(TAB_LIMIT)
    end

    def more_statements?
      @evidence.statements.size > TAB_LIMIT
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
        ids      = statements.map(&:id)
        totals   = BankTransaction.where(reconciliation_id: ids).group(:reconciliation_id).count
        resolved = BankTransaction.where(reconciliation_id: ids, status: BankTransaction::RESOLVED_STATUSES)
                                  .group(:reconciliation_id).count
        ids.index_with { |id| [ resolved.fetch(id, 0), totals.fetch(id, 0) ] }
      end
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
      @all_needs_you ||= build_all_needs_you
    end

    def build_all_needs_you # rubocop:disable Metrics/MethodLength
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
          match:       nil,
          group:       nil,
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
          group:       nil,
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
          match:       nil,
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
            match:       nil,
            group:       nil,
            title:       I18n.t("money.needs_you.nif.title"),
            meta:        [ txn.counterparty.presence || I18n.t("money.needs_you.no_name"), format_amount(txn) ],
            actions:     [ :request_invoice ]
          )
        end
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
      parts << I18n.t("money.needs_you.nif.flag") if nif_flag
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
