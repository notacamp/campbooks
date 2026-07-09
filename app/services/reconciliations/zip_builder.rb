# frozen_string_literal: true

require "zip"

module Reconciliations
  # Builds a zip archive for a completed reconciliation, structured as:
  #
  #   statement/<original filename>      — the bank statement file
  #   debits/<YYYY-MM-DD amount doc>.ext — expense docs from confirmed debit txns
  #   credits/<YYYY-MM-DD amount doc>.ext— revenue docs from confirmed credit txns
  #   index.csv                          — full transaction log with matched filenames
  #
  # Filenames are sanitized (Unicode NFKD → strip combining marks, replace
  # forbidden chars, collapse whitespace, truncate ~150 chars, deduplicate).
  #
  # A Document confirmed on multiple transactions appears once per transaction
  # entry: the accountant wants the doc under each payment row.
  class ZipBuilder
    # Characters forbidden in zip entry names (cross-platform safe subset)
    FORBIDDEN_RE = /[\\\/:*?"<>|\x00-\x1f]/.freeze

    def initialize(reconciliation)
      @reconciliation = reconciliation
    end

    def call
      buffer = Zip::OutputStream.write_buffer do |zip|
        add_statement(zip)
        add_transaction_files(zip)
        add_index_csv(zip)
      end
      buffer.string
    end

    private

    # ── Statement file ──────────────────────────────────────────────────────────

    def add_statement(zip)
      blob = @reconciliation.statement_document.original_file.blob
      return unless blob

      filename = sanitize(blob.filename.to_s)
      zip.put_next_entry("statement/#{filename}")
      zip.write(blob.download)
    end

    # ── Transaction document files ──────────────────────────────────────────────

    def add_transaction_files(zip)
      @used_names = {}

      ordered_transactions.each do |txn|
        confirmed_matches = txn.transaction_matches.select(&:confirmed?).sort_by { |m| -m.confidence.to_f }
        confirmed_matches.each do |match|
          doc  = match.document
          dir  = txn.debit? ? "debits" : "credits"
          name = entry_filename(txn, doc)

          zip.put_next_entry("#{dir}/#{name}")
          zip.write(doc_blob(doc).download)
        end
      end
    end

    def doc_blob(doc)
      doc.processed_pdf.attached? ? doc.processed_pdf.blob : doc.original_file.blob
    end

    # Filename: YYYY-MM-DD ±amount CUR Counterparty InvoiceNo.ext
    # e.g. "2026-06-14 -45.90EUR Vodafone FT129833.pdf"
    def entry_filename(txn, doc)
      sign   = txn.debit? ? "-" : "+"
      amount = format("%.2f", txn.amount_cents.abs / 100.0)
      parts  = [
        txn.booked_on.strftime("%Y-%m-%d"),
        "#{sign}#{amount}#{txn.currency}",
        txn.counterparty.presence || doc.entity_display_name.presence,
        doc.invoice_number.presence
      ].compact.reject(&:blank?)

      raw_base = parts.join(" ")
      blob     = doc_blob(doc)
      ext      = File.extname(blob.filename.to_s).downcase.presence || ".pdf"
      base     = sanitize(raw_base).truncate(150, omission: "")

      unique_name("#{base}#{ext}")
    end

    def unique_name(name)
      @used_names ||= {}
      return name unless @used_names.key?(name)

      # Append -2, -3 … on collision
      ext  = File.extname(name)
      base = File.basename(name, ext)
      n    = 2
      n += 1 while @used_names.key?("#{base}-#{n}#{ext}")
      name = "#{base}-#{n}#{ext}"
      @used_names[name] = true
      name
    ensure
      @used_names[name] ||= true
    end

    # ── index.csv ───────────────────────────────────────────────────────────────

    CSV_HEADERS = %w[
      position booked_on description counterparty amount currency
      status exclusion_reason matched_documents invoice_numbers
      includes_your_nif notes
    ].freeze

    def add_index_csv(zip)
      company_nif = @reconciliation.workspace.company_nif.presence

      csv_data = CSV.generate(force_quotes: true) do |csv|
        csv << CSV_HEADERS

        ordered_transactions.each do |txn|
          confirmed = txn.transaction_matches.select(&:confirmed?).sort_by { |m| -m.confidence.to_f }

          matched_filenames  = confirmed.map { |m| entry_filename_for_csv(txn, m.document) }
          invoice_numbers    = confirmed.filter_map { |m| m.document.invoice_number.presence }
          nif_col            = nif_column(txn, confirmed, company_nif)
          notes              = build_notes(txn, confirmed)

          csv << [
            txn.position,
            txn.booked_on.iso8601,
            txn.description,
            txn.counterparty.presence || "",
            format("%.2f", txn.amount_cents / 100.0),
            txn.currency,
            human_status(txn),
            human_exclusion_reason(txn),
            matched_filenames.join("; "),
            invoice_numbers.join("; "),
            nif_col,
            notes
          ]
        end
      end

      zip.put_next_entry("index.csv")
      zip.write(csv_data)
    end

    def entry_filename_for_csv(txn, doc)
      dir  = txn.debit? ? "debits" : "credits"
      name = entry_filename(txn, doc)
      "#{dir}/#{name}"
    end

    def nif_column(txn, confirmed_matches, company_nif)
      return "" if company_nif.blank?
      return "" if confirmed_matches.empty?

      statuses = confirmed_matches.map { |m| m.document.nif_status(company_nif) }.compact
      return "" if statuses.empty?

      if statuses.all? { |s| s == :ok }
        I18n.t("reconciliations.zip_builder.nif_yes")
      elsif statuses.any? { |s| s == :mismatch }
        I18n.t("reconciliations.zip_builder.nif_mismatch")
      else
        I18n.t("reconciliations.zip_builder.nif_missing")
      end
    end

    def build_notes(txn, confirmed_matches)
      notes = []
      if confirmed_matches.any? { |m| m.match_reasons&.dig("cross_reconciliation_warning") }
        notes << I18n.t("reconciliations.zip_builder.cross_reconciliation_note")
      end
      notes.join("; ")
    end

    def human_status(txn)
      I18n.t("activerecord.attributes.bank_transaction.statuses.#{txn.status}")
    end

    def human_exclusion_reason(txn)
      return "" if txn.exclusion_reason.blank?

      I18n.t("reconciliations.bank_transactions.exclusion_reasons.#{txn.exclusion_reason}",
             default: txn.exclusion_reason.humanize)
    end

    # ── Helpers ─────────────────────────────────────────────────────────────────

    def ordered_transactions
      @ordered_transactions ||= @reconciliation.bank_transactions.ordered
                                               .includes(transaction_matches: :document)
                                               .to_a
    end

    # Strip combining marks (accents → base letters for filenames), replace
    # forbidden characters, collapse whitespace.
    def sanitize(raw)
      # NFKD decomposes combined characters (é → e + combining accent); then
      # we drop the combining-mark category (Mn).
      normalized = raw.unicode_normalize(:nfkd).gsub(/\p{Mn}/, "")
      normalized.gsub(FORBIDDEN_RE, " ").squeeze(" ").strip
    end
  end
end
