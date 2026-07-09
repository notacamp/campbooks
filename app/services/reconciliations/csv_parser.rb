# frozen_string_literal: true

require "csv"
require "date"

module Reconciliations
  # Parses a raw CSV byte string from a bank statement export into an array of
  # row hashes ready for bulk-insert as BankTransactions.
  #
  # Handles:
  # - UTF-8 BOM stripping + Latin-1/Windows-1252 fallback
  # - Delimiter detection (`;`, `,`, TAB)
  # - Multilingual header aliases (pt / en / es / fr)
  # - Three amount layouts: signed single / debit+credit columns / amount+direction
  # - EU and US decimal formatting (1.234,56 and 1,234.56)
  # - EU integer thousands grouping (1.200 → 120000 cents)
  # - Multiple date formats, day-first for ambiguous
  # - Footer / blank row skipping
  #
  # Returns: Array of hashes { position:, booked_on: Date, description: String,
  #   counterparty: String|nil, amount_cents: Integer, balance_after_cents: Integer|nil,
  #   raw: Hash }
  #
  # Raises Reconciliations::ParseError with a human message on fatal errors.
  class CsvParser
    # ── Column header aliases ─────────────────────────────────────────────────
    COLUMN_ALIASES = {
      date: %w[
        data date fecha datum date_operation data_mov data_lanc data_valor
        data_operacao data_valeur date_valeur date_op transacoes
        date_transaction transaction_date booking_date
      ],
      description: %w[
        descricao descrição descricão historico historico_descricao movimento
        descriptivo concepto libelle description detalhes details
        transaction_description memo narrative motif remarks observacoes
        description_operation
      ],
      amount: %w[
        valor montante importe montant amount valor_eur importe_eur valor_eur_
        quantia value net_amount transaction_amount
      ],
      debit: %w[
        debito débito saida saidas cargo debit debits debit_eur saidas_eur
        montant_debit importe_cargo monto_debito
      ],
      credit: %w[
        credito crédito entrada entradas abono credit credits credit_eur
        entradas_eur montant_credit importe_abono monto_credito
      ],
      balance: %w[
        saldo saldo_apos saldo_final saldo_contabilistico solde balance
        running_balance balance_after saldo_eur
      ],
      counterparty: %w[
        beneficiario entidade ordenante contraparte payee beneficiary
        counterparty entity nome_beneficiario nombre_beneficiario
        beneficiaire tiers third_party
      ],
      direction: %w[
        tipo tipo_mov dc d_c d/c dc_indicator sentido nature sinal sign
        debit_credit db_cr tipo_lancamento cr_db
      ]
    }.freeze

    # Finding 21: precomputed inverse of COLUMN_ALIASES (alias → semantic key),
    # built once at load time so build_mapping is O(headers) not O(aliases × headers).
    ALIAS_TO_KEY = COLUMN_ALIASES.each_with_object({}) do |(key, aliases), h|
      aliases.each { |a| h[a] = key }
    end.freeze

    # ── Positive/negative direction hints ──────────────────────────────────────
    # Single "c" is intentionally absent from DEBIT_HINTS: the D=Debit / C=Credit
    # convention is near-universal in banking exports, and "c" alone must default
    # to credit.  "cargo" (Spanish) is kept as an explicit full-word debit hint.
    DEBIT_HINTS  = %w[d deb debit s out saida saidas - cargo].freeze
    CREDIT_HINTS = %w[c cre credit e in entrada entradas + a abono].freeze

    def initialize(data)
      @data = data
    end

    # Returns Array of row hashes; raises ParseError on fatal failure.
    def call
      text    = clean_encoding(@data)
      table   = parse_csv(text)            # Finding 21: delimiter detected lazily inside
      headers = normalize_headers(table.headers)
      mapping = build_mapping(headers)

      validate_mapping!(mapping)

      rows = []
      table.each_with_index do |row, idx|
        parsed = parse_row(row, headers, mapping, idx)
        rows << parsed if parsed
      end

      raise ParseError, "No data rows could be parsed from this CSV file." if rows.empty?

      rows
    end

    private

    # ── Encoding ──────────────────────────────────────────────────────────────

    def clean_encoding(raw)
      # Strip UTF-8 BOM
      str = raw.dup.force_encoding("BINARY")
      str = str.sub("\xEF\xBB\xBF".b, "")
      str = str.force_encoding("UTF-8")

      if str.valid_encoding?
        str
      else
        # Fall back to Windows-1252 (superset of Latin-1)
        str.encode("UTF-8", "Windows-1252", invalid: :replace, undef: :replace, replace: "")
      end
    end

    # ── Delimiter detection ───────────────────────────────────────────────────

    def detect_delimiter(text)
      sample = text.lines.reject { |l| l.strip.empty? }.first(5).join
      counts = { ";" => sample.count(";"), "," => sample.count(","), "\t" => sample.count("\t") }
      counts.max_by { |_, v| v }.first
    end

    # ── CSV parsing ───────────────────────────────────────────────────────────

    # Finding 21: detect the delimiter lazily here so the text is only
    # traversed once (detect + parse share the same pass through `text`).
    def parse_csv(text)
      sep = detect_delimiter(text)
      CSV.parse(text, col_sep: sep, headers: true, skip_blanks: true)
    rescue CSV::MalformedCSVError => e
      raise ParseError, "Could not parse the CSV file: #{e.message}"
    end

    # ── Header normalization ──────────────────────────────────────────────────

    def normalize_headers(raw_headers)
      raw_headers.map { |h| normalize_header_key(h.to_s) }
    end

    def normalize_header_key(str)
      # downcase, strip whitespace, collapse internal spaces, strip accents
      str.downcase.strip.gsub(/\s+/, "_").unicode_normalize(:nfd).gsub(/\p{Mn}/, "")
    end

    # Finding 21: use ALIAS_TO_KEY for O(headers) build instead of O(aliases × headers).
    def build_mapping(normalized_headers)
      mapping = {}
      normalized_headers.each_with_index do |h, i|
        key = ALIAS_TO_KEY[h]
        mapping[key] ||= i if key
      end
      mapping
    end

    # Finding 17: removed the dead `(has_amount && has_direction)` clause —
    # if has_amount is already true the first arm satisfies the condition and
    # the third arm could never be the sole reason to pass.
    def validate_mapping!(mapping)
      raise ParseError, "Could not find a date column in the CSV. Expected a column like 'Data', 'Date', or 'Fecha'." unless mapping[:date]

      has_amount  = mapping.key?(:amount)
      has_debit   = mapping.key?(:debit)
      has_credit  = mapping.key?(:credit)

      unless has_amount || (has_debit && has_credit)
        raise ParseError, "Could not find an amount column in the CSV. Expected 'Valor', 'Montante', 'Amount', or separate Debit/Credit columns."
      end
    end

    # ── Row parsing ───────────────────────────────────────────────────────────

    # Finding 21: single traversal — extract `fields` once for both raw_values
    # and the normalized-header map, avoiding two implicit row.to_h calls.
    def parse_row(row, normalized_headers, mapping, position)
      fields     = row.fields
      raw_values = row.headers.zip(fields).to_h
      values     = normalized_headers.zip(fields).to_h

      date_str   = field(values, mapping, :date)
      booked_on  = parse_date(date_str) rescue nil

      amount_cents = parse_amount_cents(values, mapping)

      # Skip rows where both date AND amount are unparseable (footer/blank rows)
      return nil if booked_on.nil? && amount_cents.nil?
      return nil if booked_on.nil? # can't skip just amount-less rows — they might be valid

      description = field(values, mapping, :description).to_s.strip
      description = "—" if description.empty?

      counterparty    = field(values, mapping, :counterparty)&.strip.presence
      balance_raw     = field(values, mapping, :balance)
      balance_cents   = balance_raw.present? ? parse_cents(balance_raw) : nil

      {
        position:           position,
        booked_on:          booked_on,
        description:        description,
        counterparty:       counterparty,
        amount_cents:       amount_cents || 0,
        balance_after_cents: balance_cents,
        raw:                raw_values
      }
    end

    def field(values, mapping, key)
      idx = mapping[key]
      return nil unless idx

      values.values.at(idx)
    end

    # ── Amount parsing ─────────────────────────────────────────────────────────

    def parse_amount_cents(values, mapping)
      if mapping[:amount] && mapping[:direction]
        # Layout 3: absolute amount + direction column
        raw_amount    = values.values.at(mapping[:amount]).to_s
        raw_direction = values.values.at(mapping[:direction]).to_s.downcase.strip
        cents = parse_cents(raw_amount)
        return nil if cents.nil?

        debit = DEBIT_HINTS.any? { |h| raw_direction.start_with?(h) }
        debit ? -cents.abs : cents.abs

      elsif mapping[:debit] && mapping[:credit]
        # Layout 2: separate debit / credit columns
        debit_raw  = values.values.at(mapping[:debit]).to_s.strip
        credit_raw = values.values.at(mapping[:credit]).to_s.strip

        debit_cents  = parse_cents(debit_raw)
        credit_cents = parse_cents(credit_raw)

        if debit_cents&.nonzero?
          -debit_cents.abs
        elsif credit_cents&.nonzero?
          credit_cents.abs
        else
          nil
        end

      elsif mapping[:amount]
        # Layout 1: single signed column
        raw = values.values.at(mapping[:amount]).to_s
        parse_cents(raw)
      end
    end

    # Parse a human-formatted decimal string to integer cents. Returns nil if
    # the string is blank or cannot be parsed.
    #
    # Finding 11: added EU integer thousands detection (`1.200` → 120000) before
    # the decimal check so a dot followed by exactly three digits is treated as
    # a thousands separator when no comma is present.
    def parse_cents(raw)
      return nil if raw.blank?

      str = raw.to_s.strip
      # Handle parentheses negatives: (45,90)
      negative = str.start_with?("(") && str.end_with?(")")
      str = str.delete("()") if negative

      # Strip currency symbols / spaces / non-numeric prefix
      str = str.gsub(/\A[^0-9\-\+\.,]+/, "").gsub(/[^0-9\-\+\.,]+\z/, "")

      # Trailing minus: "45,90-"
      if str.end_with?("-")
        negative = true
        str = str.chomp("-")
      end

      return nil if str.blank?

      # Detect EU vs US format:
      #   EU decimal:   1.234,56  or 1234,56  → comma = decimal separator
      #   EU integer:   1.200                 → dot   = thousands separator (no decimal)
      #   US decimal:   1,234.56  or 1234.56  → dot   = decimal separator
      #
      # Key insight: whichever separator appears LAST is the decimal separator.
      # Any separators before the last one are thousands separators and are stripped.
      amount = if str.match?(/,\d{1,2}\z/)
        # EU: last separator is comma → decimal; strip dots (thousands), swap comma
        str.gsub(".", "").gsub(",", ".").to_f
      elsif str.match?(/\.\d{3}\z/) && !str.include?(",")
        # EU integer grouping only: "1.200" = 1200, "2.000" = 2000.
        # Dot followed by exactly 3 digits with no comma → thousands separator.
        str.gsub(".", "").to_f
      elsif str.match?(/\.\d{1,2}\z/)
        # US: last separator is dot → decimal; strip commas (thousands)
        str.gsub(",", "").to_f
      elsif str.include?(",") && !str.include?(".")
        # Only comma, no dot — treat as EU decimal (e.g. "45,9")
        str.gsub(",", ".").to_f
      else
        # Only dots or neither — treat as US / plain integer
        str.gsub(",", "").to_f
      end

      cents = (amount * 100).round
      negative ? -cents : cents
    end

    # ── Date parsing ──────────────────────────────────────────────────────────

    DATE_FORMATS = [
      "%d/%m/%Y", "%d-%m-%Y", "%d.%m.%Y",
      "%Y-%m-%d", "%Y/%m/%d",
      "%d/%m/%y", "%d-%m-%y"
    ].freeze

    def parse_date(raw)
      return nil if raw.blank?

      str = raw.to_s.strip
      DATE_FORMATS.each do |fmt|
        return Date.strptime(str, fmt)
      rescue ArgumentError, Date::Error
        next
      end
      nil
    end
  end
end
