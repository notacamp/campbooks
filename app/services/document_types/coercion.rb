# frozen_string_literal: true

module DocumentTypes
  # Shared type coercion used by DocumentTypes::Schema::Field (write-time
  # normalization) and the Documents::ExtractedFields concern (reader/writer
  # pair generation).
  #
  # Call as module methods: Coercion.coerce(type, value), Coercion.read(type, meta, key).
  # Private helpers are callable within the module via extend self.
  module Coercion
    extend self

    # Coerce +value+ to the storage form for the given +type+.
    # Returns nil for nil, blank strings, or uncoerceable garbage.
    # +enum_values+ must be provided when +type+ is :enum.
    def coerce(type, value, enum_values: nil) # rubocop:disable Metrics/MethodLength, Metrics/CyclomaticComplexity
      return nil if value.nil?
      return nil if value.respond_to?(:strip) && value.strip.empty?

      case type
      when :string          then value.to_s.strip
      when :integer, :money then coerce_integer(value)
      when :number          then coerce_number(value)
      when :date            then coerce_date(value)
      when :enum
        str = value.to_s.strip
        enum_values&.include?(str) ? str : nil
      when :boolean         then coerce_boolean(value)
      else                       value.to_s.strip
      end
    end

    # Read +key+ from +metadata+ hash as the typed Ruby value for +type+.
    # Returns nil when metadata is nil or the key is absent/unreadable.
    def read(type, metadata, key) # rubocop:disable Metrics/MethodLength, Metrics/CyclomaticComplexity
      return nil if metadata.nil?

      raw = metadata[key.to_s]
      return nil if raw.nil?

      case type
      when :date
        begin
          Date.iso8601(raw.to_s)
        rescue ArgumentError, TypeError
          nil
        end
      when :integer, :money then read_integer(raw)
      when :number
        case raw
        when Numeric then raw.to_f
        when String  then Float(raw.strip, exception: false)
        end
      when :boolean
        case raw
        when true,  "true",  "1" then true
        when false, "false", "0" then false
        end
      else raw
      end
    end

    private

    def coerce_integer(value)
      case value
      when Integer then value
      when Float   then value.to_i
      when String
        str = value.strip
        str.match?(/\A-?\d+(\.\d+)?\z/) ? str.to_f.to_i : nil
      end
    end

    def read_integer(raw)
      case raw
      when Integer then raw
      when Float   then raw.to_i
      when String  then Integer(raw.strip, exception: false)
      end
    end

    def coerce_number(value)
      case value
      when Numeric then value.to_f
      when String  then Float(value.strip, exception: false)
      end
    end

    def coerce_date(value)
      case value
      when Date           then value.iso8601
      when Time, DateTime then value.to_date.iso8601
      when String
        str = value.strip
        return nil if str.empty?

        begin
          Date.parse(str).iso8601
        rescue ArgumentError, TypeError
          nil
        end
      end
    end

    def coerce_boolean(value)
      case value
      when true,  "true",  "1" then true
      when false, "false", "0" then false
      end
    end
  end
end
