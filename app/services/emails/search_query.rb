# frozen_string_literal: true

require "strscan"

module Emails
  # Pure, dependency-free parser for the inbox search box string. Extracts
  # Gmail-style `modifier:value` tokens from raw input and separates them from
  # free text.  No ActiveRecord, no I/O — safe to call anywhere.
  #
  #   parsed = Emails::SearchQuery.parse("invoice from:acme has:attachment")
  #   parsed.text      # => "invoice"
  #   parsed.filters   # => { sender: ["acme"], has_attachment: true }
  #   parsed.filters?  # => true
  class SearchQuery
    VALID_PRIORITIES = %w[low medium high].freeze
    VALID_HAS        = %w[attachment].freeze
    VALID_IS         = %w[unread read pinned].freeze

    # Modifier keys that accumulate arrays (deduped, case preserved).
    ARRAY_KEYS = %i[sender domain to subject category priority tag_names account].freeze

    # Modifier keys that are single-value (last occurrence wins).
    SINGLE_KEYS = %i[date_from date_to folder].freeze

    # @param raw [String, nil]
    # @return [SearchQuery]
    def self.parse(raw)
      new(raw)
    end

    def initialize(raw)
      @raw    = raw.to_s
      @filters = {}
      @text_tokens = []
      parse!
    end

    # Free text with all modifier tokens removed, whitespace squished.
    def text
      @text_tokens.join(" ")
    end

    # Hash of parsed filters — only keys that produced a value are present.
    def filters
      @filters
    end

    # True when at least one modifier was parsed successfully.
    def filters?
      @filters.any?
    end

    private

    # One pass over the raw string. Splits on whitespace but respects
    # double-quoted values that may contain spaces.
    #
    # Token grammar: /\A([a-z]+):("([^"]*)"|(\S*))\z/i
    # — modifier name is case-insensitive letters only
    # — value is either double-quoted (may have spaces, quotes stripped) or
    #   bare (up to the next whitespace)
    # A dangling modifier with an empty value (e.g. "from:" or 'from:""') is
    # silently dropped — it occurs transiently while the user is still typing.
    def parse!
      tokens = tokenize(@raw)
      tokens.each do |token|
        next if handle_modifier(token) == :consumed
        @text_tokens << token
      end
    end

    # Tokenizes respecting quoted strings.  A quoted segment is returned as a
    # single token including its surrounding quotes so handle_modifier can
    # detect `key:"quoted value"` patterns.
    def tokenize(raw)
      tokens = []
      scanner = StringScanner.new(raw)
      until scanner.eos?
        scanner.skip(/\s+/)
        break if scanner.eos?

        # key:"quoted value" — capture the whole `key:` + `"value"` as one token
        if (tok = scanner.scan(/[a-zA-Z]+:"[^"]*"/))
          tokens << tok
        # key:bare-value
        elsif (tok = scanner.scan(/[a-zA-Z]+:\S*/))
          tokens << tok
        # bare word (no colon)
        elsif (tok = scanner.scan(/\S+/))
          tokens << tok
        else
          scanner.getch
        end
      end
      tokens
    end

    # @return [:consumed, nil]  :consumed means the token was a recognized modifier
    def handle_modifier(token)
      # Must match `word:` — if there is no colon or the part before the colon
      # contains non-letters, treat as plain text.
      colon = token.index(":")
      return unless colon&.positive?

      modifier  = token[0, colon].downcase
      raw_value = token[colon + 1..]

      # Strip surrounding double quotes.
      value = if raw_value.start_with?('"') && raw_value.end_with?('"') && raw_value.length > 1
        raw_value[1..-2]
      else
        raw_value
      end

      # Drop dangling/empty modifier ("from:" or 'from:""').
      return :consumed if value.blank?

      case modifier
      when "from"
        if value.start_with?("@")
          push_array(:domain, value.delete_prefix("@"))
        else
          push_array(:sender, value)
        end

      when "to"
        push_array(:to, value)

      when "subject"
        push_array(:subject, value)

      when "has"
        if VALID_HAS.include?(value.downcase)
          @filters[:has_attachment] = true
        else
          return nil # unknown has: → plain text
        end

      when "is"
        case value.downcase
        when "unread" then @filters[:unread]  = true
        when "read"   then @filters[:read]    = true
        when "pinned" then @filters[:pinned]  = true
        else
          return nil # unknown is: → plain text
        end

      when "after"
        date = parse_date(value)
        return :consumed if date.nil? # invalid date → dropped, not left in text
        @filters[:date_from] = date

      when "before"
        date = parse_date(value)
        return :consumed if date.nil?
        @filters[:date_to] = date

      when "tag", "label"
        push_array(:tag_names, value)

      when "folder", "in"
        @filters[:folder] = value

      when "category"
        push_array(:category, value)

      when "priority"
        if VALID_PRIORITIES.include?(value.downcase)
          push_array(:priority, value.downcase)
        else
          return nil # unknown priority → plain text
        end

      when "account"
        push_array(:account, value)

      else
        return nil # unknown modifier → plain text
      end

      :consumed
    end

    # Accumulate into an array filter, deduping (case-preserved).
    def push_array(key, value)
      @filters[key] ||= []
      @filters[key] << value unless @filters[key].include?(value)
    end

    # Accept YYYY-MM-DD and YYYY/MM/DD; invalid dates return nil.
    def parse_date(raw)
      # Normalize slashes to dashes
      normalized = raw.to_s.tr("/", "-")
      return nil unless normalized.match?(/\A\d{4}-\d{2}-\d{2}\z/)
      Date.strptime(normalized, "%Y-%m-%d").strftime("%Y-%m-%d")
    rescue ArgumentError
      nil
    end
  end
end
