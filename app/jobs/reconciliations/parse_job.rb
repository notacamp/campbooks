# frozen_string_literal: true

module Reconciliations
  # Downloads the Reconciliation's statement Document, parses it (CSV only in
  # PR 1), bulk-inserts BankTransactions, and marks status :ready.
  #
  # Error handling:
  # - Reconciliations::ParseError → status :failed + parse_error, no re-raise
  # - Other StandardError          → status :failed then re-raise so retry_on fires
  class ParseJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    # Registry of content-type → parser class.
    #
    # CSV parsers take `new(data).call` (raw bytes).
    # PDF parser is document-aware and takes `new(document).call`; the :pdf sentinel
    # signals that, so parser_for can dispatch differently.
    PARSERS = {
      "text/csv"        => Reconciliations::CsvParser,
      "application/csv" => Reconciliations::CsvParser,
      "application/pdf" => :pdf
    }.freeze

    def perform(reconciliation_id)
      @reconciliation = Reconciliation.find(reconciliation_id)
      Current.workspace = @reconciliation.workspace

      mark_parsing!

      document = @reconciliation.statement_document
      data     = download_statement
      rows     = parse(data, document)

      ActiveRecord::Base.transaction do
        @reconciliation.bank_transactions.delete_all

        # Finding 3: derive period/integrity and apply any sign-flip BEFORE
        # building the insert records, so the persisted amounts are correct.
        update_period_and_integrity!(rows)

        records = rows.map do |r|
          {
            id:                  SecureRandom.uuid,
            reconciliation_id:   @reconciliation.id,
            workspace_id:        @reconciliation.workspace_id,
            position:            r[:position],
            booked_on:           r[:booked_on],
            description:         r[:description],
            counterparty:        r[:counterparty],
            amount_cents:        r[:amount_cents],
            currency:            @reconciliation.currency,
            balance_after_cents: r[:balance_after_cents],
            raw_data:            r[:raw].to_json,
            status:              BankTransaction.statuses[:unmatched],
            created_at:          Time.current,
            updated_at:          Time.current
          }
        end

        BankTransaction.insert_all!(records) if records.any?
        # Transition to :matching — MatchJob will set :ready when done.
        @reconciliation.update!(status: :matching)
      end

      broadcast_update!
      Reconciliations::MatchJob.perform_later(@reconciliation.id)

    rescue Reconciliations::ParseError => e
      @reconciliation&.update_columns(
        status:      Reconciliation.statuses[:failed],
        parse_error: e.message,
        updated_at:  Time.current
      )
      broadcast_update!
      # Do not re-raise — ParseError is a user-fixable data problem, not an
      # infrastructure error.  retry_on would only hammer the same bad file.

    rescue StandardError => e
      @reconciliation&.update_columns(
        status:      Reconciliation.statuses[:failed],
        parse_error: "Internal error: #{e.class}: #{e.message.first(500)}",
        updated_at:  Time.current
      )
      broadcast_update!
      raise # let retry_on fire

    ensure
      Current.workspace = nil
    end

    private

    def mark_parsing!
      @reconciliation.update!(status: :parsing)
      broadcast_update!
    end

    # Download the raw bytes of the statement file.
    def download_statement
      blob = @reconciliation.statement_document.original_file.blob
      blob.open { |f| f.read }
    end

    # Determine the correct parser for this statement and run it.
    # Raises ParseError if no parser is registered for the format.
    def parse(data, document)
      blob  = document.original_file.blob
      klass = parser_for(blob)

      if klass.nil?
        raise Reconciliations::ParseError,
              I18n.t("reconciliations.parse_job.unsupported_format")
      end

      if klass == :pdf
        parse_pdf(document)
      else
        klass.new(data).call
      end
    end

    # Resolve content-type and filename extension to a parser class or sentinel.
    # Returns nil when the format is unsupported.
    def parser_for(blob)
      content_type = blob.content_type.to_s
      filename     = blob.filename.to_s.downcase

      if PARSERS.key?(content_type)
        return PARSERS[content_type]
      end
      return Reconciliations::CsvParser if filename.end_with?(".csv")
      return :pdf if filename.end_with?(".pdf")

      nil
    end

    # Parse a PDF via the AI provider. Gates on document AI being configured.
    def parse_pdf(document)
      workspace = @reconciliation.workspace
      unless Ai::ProviderSetup.configured?(workspace, :documents)
        raise Reconciliations::ParseError,
              I18n.t("reconciliations.parse_job.no_ai_for_pdf")
      end

      result = Ai::BankStatementParser.new(document).call

      # Fill reconciliation header fields from parser output when blank.
      fill_header_from_ai(result)

      # Convert AI transactions to the same row format CsvParser returns.
      ai_transactions_to_rows(result["transactions"] || [])
    end

    # Apply AI-parsed header fields to the reconciliation when they are blank.
    def fill_header_from_ai(result)
      attrs = {}
      attrs[:bank_name]              = result["bank_name"]             if @reconciliation.bank_name.blank? && result["bank_name"].present?
      attrs[:currency]               = result["currency"]              if result["currency"].present?
      attrs[:period_start]           = Date.parse(result["period_start"])  rescue nil if @reconciliation.period_start.blank? && result["period_start"].present?
      attrs[:period_end]             = Date.parse(result["period_end"])    rescue nil if @reconciliation.period_end.blank? && result["period_end"].present?
      attrs[:opening_balance_cents]  = result["opening_balance_cents"].to_i if @reconciliation.opening_balance_cents.blank? && result["opening_balance_cents"].present?
      attrs[:closing_balance_cents]  = result["closing_balance_cents"].to_i if @reconciliation.closing_balance_cents.blank? && result["closing_balance_cents"].present?
      @reconciliation.update!(attrs) if attrs.any?
    end

    # Map AI transaction hashes to the internal row format.
    def ai_transactions_to_rows(transactions)
      transactions.each_with_index.map do |t, idx|
        date = begin
          Date.parse(t["date"].to_s)
        rescue
          nil
        end
        next nil if date.nil?

        amount_decimal = t["amount"].to_f
        amount_cents   = (amount_decimal * 100).round
        bal_decimal    = t["balance_after"]
        bal_cents      = bal_decimal.present? ? (bal_decimal.to_f * 100).round : nil

        {
          position:            idx + 1,
          booked_on:           date,
          description:         t["description"].to_s.strip.presence || "Transaction #{idx + 1}",
          counterparty:        t["counterparty"].presence,
          amount_cents:        amount_cents,
          balance_after_cents: bal_cents,
          raw:                 t
        }
      end.compact
    end

    # After a successful parse, derive period_start/end and run the integrity
    # check if opening/closing balance bookends are known.
    #
    # Finding 3/4: the sign-flip is applied to `rows` in this method; because
    # we call it BEFORE building the insert records, the persisted amounts get
    # the corrected signs. The integrity warning message is built with the
    # post-flip expected value so the numbers are accurate.
    def update_period_and_integrity!(rows)
      return if rows.empty?

      dates = rows.map { |r| r[:booked_on] }.compact.sort

      attrs = {
        period_start:              dates.first,
        period_end:                dates.last,
        integrity_warning:         false,
        integrity_warning_message: nil
      }

      opening = @reconciliation.opening_balance_cents
      closing = @reconciliation.closing_balance_cents

      if opening.present? && closing.present?
        total = rows.sum { |r| r[:amount_cents].to_i }

        # Sign-sanity check: if the net sign contradicts closing − opening,
        # flip all signs (the bank may export debits as positive).
        expected_sign = (closing - opening) >= 0 ? :credit : :debit
        parsed_sign   = total >= 0 ? :credit : :debit

        if expected_sign != parsed_sign
          Rails.logger.info("[Reconciliations::ParseJob] sign-flip applied for reconciliation #{@reconciliation.id}")
          rows.each { |r| r[:amount_cents] = -r[:amount_cents] }
          total = -total # reflect the flip in the running total
        end

        # Balance check uses post-flip total so the message is accurate.
        expected_closing = opening + total
        diff = (expected_closing - closing).abs

        if diff > 1 # 1-cent tolerance for rounding
          currency = @reconciliation.currency
          attrs[:integrity_warning] = true
          attrs[:integrity_warning_message] =
            I18n.t("reconciliations.parse_job.integrity_mismatch",
                   expected: Money.new(expected_closing, currency).format,
                   actual:   Money.new(closing, currency).format)
        end
      end

      @reconciliation.assign_attributes(attrs)
    end

    # Broadcast a Turbo Streams update so the show page refreshes live.
    # Finding 5: broadcast_update_to (not replace_to) so the div#reconciliation_content
    # shell is preserved and only its inner HTML is swapped.
    # Finding 9: rendered inside I18n.with_locale so the partial sees the right locale.
    def broadcast_update!
      locale = @reconciliation.created_by&.locale.presence || I18n.default_locale
      transaction_count = @reconciliation.bank_transactions.count
      html = I18n.with_locale(locale) do
        ApplicationController.render(
          partial: "reconciliations/show_content",
          locals:  {
            reconciliation: @reconciliation,
            transactions:   @reconciliation.bank_transactions.ordered
                                           .includes(transaction_matches: :document).limit(50),
            next_page:      (transaction_count > 50 ? 2 : nil)
          },
          layout:  false
        )
      end
      Turbo::StreamsChannel.broadcast_update_to(
        "reconciliation_#{@reconciliation.id}",
        target: "reconciliation_content",
        html:   html
      )
    rescue StandardError => e
      Rails.logger.warn("[Reconciliations::ParseJob] broadcast failed: #{e.class}: #{e.message}")
    end
  end
end
