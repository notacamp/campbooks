# frozen_string_literal: true

require "strscan"

module Documents
  # Pure, dependency-free parser for the Files search box string. Extracts
  # Gmail-style `modifier:value` tokens (and `amount>N` comparators) from raw
  # input and separates them from free text. No ActiveRecord, no I/O — safe to
  # call anywhere.
  #
  #   parsed = Documents::SearchQuery.parse("invoice type:receipt vendor:EDP amount>100")
  #   parsed.text      # => "invoice"
  #   parsed.filters   # => { type_names: ["receipt"], entities: ["EDP"], amount_min_cents: 10000 }
  #   parsed.filters?  # => true
  #
  # Mirrors Emails::SearchQuery closely; differences are the modifier vocabulary,
  # the amount comparator token form, and month-granular date parsing.
  class SearchQuery
    # ── Allowlists (kept inline — no AR dependency) ──────────────────────────
    VALID_CATEGORIES    = %w[accounting legal insurance vehicles identification correspondence other].freeze
    VALID_REVIEW_STATUS = %w[pending approved rejected].freeze
    VALID_AI_STATUS     = %w[failed processing].freeze
    VALID_IS            = (%w[starred] + VALID_REVIEW_STATUS + VALID_AI_STATUS).freeze
    VALID_SOURCES       = %w[email upload notion sent].freeze
    VALID_EXPENSE_CATS  = %w[travel meals office_supplies utilities rent software
                             professional_services equipment marketing other].freeze

    # Normalise UI-friendly source names to the Document#source enum values.
    SOURCE_MAP = {
      "email"  => "email",
      "upload" => "manual_upload",
      "notion" => "notion",
      "sent"   => "sent_email"
    }.freeze

    # Modifier keys whose values accumulate into arrays (deduped).
    ARRAY_KEYS = %i[type_names categories sources entities numbers expense_categories].freeze

    # Modifier keys that are single-value (last occurrence wins).
    SINGLE_KEYS = %i[review_status ai_status date_from date_to
                     amount_min_cents amount_max_cents folder_name].freeze

    # @param raw [String, nil]
    # @return [SearchQuery]
    def self.parse(raw)
      new(raw)
    end

    def initialize(raw)
      @raw         = raw.to_s
      @filters     = {}
      @text_tokens = []
      parse!
    end

    # Free text with all modifier/comparator tokens removed, whitespace squished.
    def text
      @text_tokens.join(" ")
    end

    # Hash of parsed filters — only keys that produced a value are present.
    attr_reader :filters

    # True when at least one modifier was parsed successfully.
    def filters?
      @filters.any?
    end

    private

    def parse!
      tokens = tokenize(@raw)
      tokens.each do |token|
        next if handle_token(token) == :consumed
        @text_tokens << token
      end
    end

    # One pass over the raw string, respecting double-quoted values and the
    # comparator token form (`amount>100`, `amount>=99.5`).
    #
    # Scan order (first match wins):
    #   1. key:"quoted value"
    #   2. amount comparator: word[<>]=?value
    #   3. key:bare-value
    #   4. bare word
    def tokenize(raw)
      tokens  = []
      scanner = StringScanner.new(raw)
      until scanner.eos?
        scanner.skip(/\s+/)
        break if scanner.eos?

        if (tok = scanner.scan(/[a-zA-Z]+:"[^"]*"/))         # key:"quoted value"
          tokens << tok
        elsif (tok = scanner.scan(/[a-zA-Z]+[<>]=?\S+/))     # amount>100 / word<=x
          tokens << tok
        elsif (tok = scanner.scan(/[a-zA-Z]+:\S*/))           # key:bare-value
          tokens << tok
        elsif (tok = scanner.scan(/\S+/))                     # plain word
          tokens << tok
        else
          scanner.getch
        end
      end
      tokens
    end

    # Dispatch to either the comparator handler or the colon-modifier handler.
    # Returns :consumed when the token was fully handled; nil to leave it as text.
    def handle_token(token)
      # Comparator form: word>value, word>=value, word<value, word<=value
      if (m = token.match(/\A([a-zA-Z]+)([<>]=?)(.+)\z/))
        return handle_comparator(m[1].downcase, m[2], m[3])
      end

      handle_modifier(token)
    end

    # Only `amount` is recognised in comparator form; everything else stays as
    # plain text. Invalid amounts are silently dropped (token consumed but not filed).
    def handle_comparator(key, op, raw_value)
      return nil unless key == "amount"

      cents = parse_amount_cents(raw_value)
      return :consumed if cents.nil? # invalid → dropped, not left in text

      case op
      when ">", ">="
        @filters[:amount_min_cents] = cents
      when "<", "<="
        @filters[:amount_max_cents] = cents
      end

      :consumed
    end

    # @return [:consumed, nil] — :consumed removes the token from text
    def handle_modifier(token)
      colon = token.index(":")
      return unless colon&.positive?

      modifier  = token[0, colon].downcase
      raw_value = token[colon + 1..]

      # Strip surrounding double quotes from the value.
      value = if raw_value.start_with?('"') && raw_value.end_with?('"') && raw_value.length > 1
        raw_value[1..-2]
      else
        raw_value
      end

      # Dangling modifier ("type:" or 'type:""') — silently consumed.
      return :consumed if value.blank?

      case modifier
      when "type"
        push_array(:type_names, value)

      when "category"
        if VALID_CATEGORIES.include?(value.downcase)
          push_array(:categories, value.downcase)
        else
          return nil # unknown category → plain text
        end

      when "source"
        mapped = SOURCE_MAP[value.downcase]
        if mapped
          push_array(:sources, mapped)
        else
          return nil # unknown source → plain text
        end

      when "is"
        case value.downcase
        when "starred"
          @filters[:starred] = true
        when *VALID_REVIEW_STATUS
          @filters[:review_status] = value.downcase
        when *VALID_AI_STATUS
          @filters[:ai_status] = value.downcase
        else
          return nil # unknown is: → plain text
        end

      when "vendor", "entity"
        push_array(:entities, value)

      when "number", "ref"
        push_array(:numbers, value)

      when "expense"
        if VALID_EXPENSE_CATS.include?(value.downcase)
          push_array(:expense_categories, value.downcase)
        else
          return nil # unknown expense category → plain text
        end

      when "after"
        date_str, _granularity = parse_date(value)
        return :consumed if date_str.nil? # invalid date → dropped
        @filters[:date_from] = date_str

      when "before"
        date_str, granularity = parse_date(value)
        return :consumed if date_str.nil? # invalid date → dropped
        if granularity == :month
          # Month-granular before: → bump to end of that month
          date_str = Date.parse(date_str).end_of_month.strftime("%Y-%m-%d")
        end
        @filters[:date_to] = date_str

      when "folder", "in"
        @filters[:folder_name] = value

      else
        return nil # unknown modifier → plain text
      end

      :consumed
    end

    def push_array(key, value)
      @filters[key] ||= []
      @filters[key] << value unless @filters[key].include?(value)
    end

    # Parse a date string accepting YYYY-MM-DD, YYYY/MM/DD (day granularity) and
    # YYYY-MM / YYYY/MM (month granularity — caller decides how to apply the bound).
    # Returns [iso_date_string, :day | :month] or nil for an unrecognised / invalid date.
    def parse_date(raw)
      normalized = raw.to_s.tr("/", "-")

      if normalized.match?(/\A\d{4}-\d{2}\z/)
        # Month granularity: YYYY-MM
        date = Date.strptime("#{normalized}-01", "%Y-%m-%d")
        [ date.strftime("%Y-%m-%d"), :month ]
      elsif normalized.match?(/\A\d{4}-\d{2}-\d{2}\z/)
        date = Date.strptime(normalized, "%Y-%m-%d")
        [ date.strftime("%Y-%m-%d"), :day ]
      end
    rescue ArgumentError
      nil
    end

    # Parse an amount string (EUR value) to integer cents.
    #
    # Accepted formats:
    #   "100"        → 10000
    #   "100.50"     → 10050
    #   "100,50"     → 10050  (single comma → decimal separator)
    #   "1.234,56"   → 123456 (EU format: dots thousands, comma decimal)
    #   "€ 1 234,56" → 123456 (currency symbol and spaces stripped)
    #
    # Returns nil for blank or unparseable input (caller silently drops the token).
    def parse_amount_cents(raw)
      s = raw.to_s.gsub(/[€$\s]/, "")
      return nil if s.blank?

      has_dot   = s.include?(".")
      has_comma = s.include?(",")

      normalized = if has_dot && has_comma
        # EU format: dots as thousands separators, comma as decimal point
        s.gsub(".", "").gsub(",", ".")
      elsif has_comma && !has_dot
        # Comma as decimal separator only
        s.gsub(",", ".")
      else
        s
      end

      (BigDecimal(normalized) * 100).round.to_i
    rescue ArgumentError, TypeError
      nil
    end
  end
end
