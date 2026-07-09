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

    def perform(reconciliation_id)
      @reconciliation = Reconciliation.find(reconciliation_id)
      Current.workspace = @reconciliation.workspace

      mark_parsing!

      data = download_statement
      rows = parse(data, @reconciliation.statement_document)

      ActiveRecord::Base.transaction do
        @reconciliation.bank_transactions.delete_all

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
            status:              0, # unmatched
            created_at:          Time.current,
            updated_at:          Time.current
          }
        end

        BankTransaction.insert_all!(records) if records.any?

        update_period_and_integrity!(rows)
        @reconciliation.update!(status: :ready)
      end

      broadcast_update!

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

    # Determine whether the statement is CSV or PDF and dispatch accordingly.
    def parse(data, document)
      blob          = document.original_file.blob
      content_type  = blob.content_type.to_s
      filename      = blob.filename.to_s.downcase

      if content_type.include?("csv") || filename.end_with?(".csv")
        Reconciliations::CsvParser.new(data).call
      else
        # PDF / unknown — AI parsing ships in a later PR.
        raise Reconciliations::ParseError, I18n.t("reconciliations.parse_job.pdf_not_yet_supported")
      end
    end

    # After a successful parse, derive period_start/end and run the integrity
    # check if opening/closing balance bookends are known.
    def update_period_and_integrity!(rows)
      return if rows.empty?

      dates = rows.map { |r| r[:booked_on] }.compact.sort

      attrs = {
        period_start:             dates.first,
        period_end:               dates.last,
        integrity_warning:        false,
        integrity_warning_message: nil
      }

      opening = @reconciliation.opening_balance_cents
      closing = @reconciliation.closing_balance_cents

      if opening.present? && closing.present?
        total = rows.sum { |r| r[:amount_cents].to_i }
        expected_closing = opening + total
        diff = (expected_closing - closing).abs

        if diff > 1 # 1-cent tolerance for rounding
          attrs[:integrity_warning] = true
          attrs[:integrity_warning_message] =
            I18n.t("reconciliations.parse_job.integrity_mismatch",
                   expected: format_cents(expected_closing),
                   actual:   format_cents(closing))

          # Sign-sanity check: if the net sign contradicts closing − opening,
          # flip all signs (the bank may export debits as positive).
          expected_sign = (closing - opening) >= 0 ? :credit : :debit
          parsed_sign   = total >= 0 ? :credit : :debit

          if expected_sign != parsed_sign
            Rails.logger.info("[Reconciliations::ParseJob] sign-flip applied for reconciliation #{@reconciliation.id}")
            rows.each { |r| r[:amount_cents] = -r[:amount_cents] }
          end
        end
      end

      @reconciliation.assign_attributes(attrs)
    end

    def format_cents(cents)
      format("%.2f", cents.to_f / 100)
    end

    # Broadcast a Turbo Streams replace so the show page updates live.
    def broadcast_update!
      html = ApplicationController.render(
        partial: "reconciliations/show_content",
        locals:  { reconciliation: @reconciliation },
        layout:  false
      )
      Turbo::StreamsChannel.broadcast_replace_to(
        "reconciliation_#{@reconciliation.id}",
        target: "reconciliation_content",
        html:   html
      )
    rescue StandardError => e
      Rails.logger.warn("[Reconciliations::ParseJob] broadcast failed: #{e.class}: #{e.message}")
    end
  end
end
