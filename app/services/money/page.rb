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
  #   page.read               # Money::Read
  #   page.needs_you          # Array<Money::NeedsYouItem>
  #   page.selected_statement # Reconciliation or nil
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

      @evidence = Money::Evidence.for(workspace)
      @ledger   = Money::Ledger.for(workspace, user, today: today, evidence: @evidence)

      # Preload groups for selected statement and pass them to Read to avoid double query.
      sel_groups = selected_statement ? Reconciliations::Groups.new(selected_statement).call : []
      @read = Money::Read.for(workspace, user,
                              today: today,
                              evidence: @evidence,
                              ledger: @ledger,
                              groups: sel_groups)
    end

    def statements
      @statements ||= @evidence.statements.first(6)
    end

    def more_statements?
      @evidence.statements.size > 6
    end

    def selected_statement
      @selected_statement ||= begin
        if @statement_id.present?
          @workspace.reconciliations.ready.find_by(id: @statement_id) ||
            @evidence.statements.first
        else
          @evidence.statements.first
        end
      end
    end

    def selected_groups
      @selected_groups ||=
        if selected_statement
          Reconciliations::Groups.new(selected_statement).call
        else
          []
        end
    end

    def needs_you
      @needs_you ||= build_needs_you
    end

    def needs_you_overflow
      all = build_all_needs_you
      [ all.size - NEEDS_YOU_CAP, 0 ].max
    end

    def missing
      @ledger.missing
    end

    private

    def build_needs_you
      all = build_all_needs_you
      all.first(NEEDS_YOU_CAP)
    end

    def build_all_needs_you # rubocop:disable Metrics/MethodLength
      stmt = @read.statement
      return [] unless stmt

      txns = stmt.bank_transactions
                 .includes(transaction_matches: :document)
                 .order(:position)
                 .to_a

      items = []

      # 1. :no_invoice — each unmatched debit
      txns.select(&:unmatched?).select(&:debit?).each do |txn|
        meta = build_no_invoice_meta(txn)
        items << NeedsYouItem.new(
          kind:        :no_invoice,
          transaction: txn,
          match:       nil,
          group:       nil,
          title:       I18n.t("money.needs_you.no_invoice.title"),
          meta:        meta,
          actions:     [ :resolve ]
        )
      end

      # 2. :review — each suggested transaction with its best match
      txns.select(&:suggested?).each do |txn|
        match = txn.transaction_matches.select(&:suggested?).max_by(&:confidence)
        next unless match

        nif_flag = @company_nif.present? && match.document&.nif_status(@company_nif)&.in?(%i[missing mismatch])
        meta = build_review_meta(txn, match, nif_flag)
        items << NeedsYouItem.new(
          kind:        :review,
          transaction: txn,
          match:       match,
          group:       nil,
          title:       I18n.t("money.needs_you.review.title"),
          meta:        meta,
          actions:     [ :change, :confirm ]
        )
      end

      # 3. :partial — each partial group (from selected_groups)
      selected_groups.select { |g| g.kind == :partial }.each do |group|
        txn = group.bank_transactions.first
        doc = group.documents.first
        vendor = doc&.entity_display_name || txn&.counterparty || txn&.description || ""
        meta = build_partial_meta(group)
        items << NeedsYouItem.new(
          kind:        :partial,
          transaction: txn,
          match:       nil,
          group:       group,
          title:       I18n.t("money.needs_you.partial.title", vendor: vendor),
          meta:        meta,
          actions:     [ :open_statement ]
        )
      end

      # 4. :nif — matched lines with a NIF-flagged document
      if @company_nif.present?
        txns.select(&:matched?).each do |txn|
          next unless txn.nif_flagged?(@company_nif)

          meta = [ txn.counterparty.presence || I18n.t("money.needs_you.no_name"),
                   format_amount(txn) ]
          items << NeedsYouItem.new(
            kind:        :nif,
            transaction: txn,
            match:       nil,
            group:       nil,
            title:       I18n.t("money.needs_you.nif.title"),
            meta:        meta,
            actions:     [ :request_invoice ]
          )
        end
      end

      items
    end

    def build_no_invoice_meta(txn)
      name = txn.counterparty.presence || I18n.t("money.needs_you.no_name")
      [ name, format_amount(txn), l(txn.booked_on, format: :date) ]
    end

    def build_review_meta(txn, match, nif_flag)
      pct = "#{(match.confidence * 100).round}% #{I18n.t('money.needs_you.review.likely')}"
      parts = [ txn.counterparty.presence || match.document&.entity_display_name || "",
                format_amount(txn) ]
      inv = match.document&.invoice_number
      parts << "#{I18n.t('money.what.invoice', number: inv)}" if inv.present?
      parts << pct
      parts << I18n.t("money.needs_you.nif.flag") if nif_flag
      parts
    end

    def build_partial_meta(group)
      paid       = ::Money.new(group.allocated_cents.to_i, "EUR")
      total      = ::Money.new(group.invoice_total_cents.to_i, "EUR")
      outstanding = ::Money.new(group.outstanding_cents.to_i, "EUR")
      [
        I18n.t("money.needs_you.partial.meta",
               paid:        paid.format,
               total:       total.format,
               outstanding: outstanding.format)
      ]
    end

    def format_amount(txn)
      ::Money.new(txn.amount_cents.abs, txn.try(:currency) || "EUR").format
    end

    def l(date, format:)
      I18n.l(date, format: format)
    end
  end
end
