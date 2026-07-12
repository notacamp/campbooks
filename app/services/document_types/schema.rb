# frozen_string_literal: true

module DocumentTypes
  # Wraps a raw JSONB extraction_schema hash into a typed field catalog.
  #
  # Usage:
  #   schema = DocumentTypes::Schema.new(document_type.extraction_schema)
  #   schema.fields   # => ordered array of Field structs
  #   schema.field("vendor_name")  # => Field or nil
  #
  # Schema.for(classification) is a convenience constructor that reads
  # extraction_schema from a DocumentType record (or returns an empty schema
  # when classification is nil).
  class Schema
    # ── Field struct ─────────────────────────────────────────────────────────

    Field = Struct.new(:key, :type, :label, :enum_values, :position, keyword_init: true) do # rubocop:disable Metrics/BlockLength
      # Legacy kind for Skim / field-set UI consumers.
      # money→:money  date→:date  enum→:enum  everything else→:text
      def kind
        case type
        when :money then :money
        when :date  then :date
        when :enum  then :enum
        else             :text
        end
      end

      # Write-time coercion: normalize +raw+ to the stored representation.
      # Returns nil for nil/blank or uncoerceable garbage.
      def coerce(raw)
        Coercion.coerce(type, raw, enum_values: enum_values)
      end

      # Read +key+ from +metadata_hash+ as the typed Ruby value.
      # Nil-safe: returns nil when metadata_hash is nil.
      def read(metadata_hash)
        Coercion.read(type, metadata_hash, key)
      end

      # ORDER BY clause fragment safe for Arel.sql(field.order_sql(:asc)).
      # The metadata key is embedded via connection.quote (never string interpolation
      # of unquoted input), so hostile keys are SQL-literal-escaped.
      # direction: :asc (default) or :desc
      def order_sql(direction = :asc) # rubocop:disable Metrics/MethodLength
        dir = direction.to_sym == :desc ? "DESC" : "ASC"
        qk  = ActiveRecord::Base.connection.quote(key)

        case type
        when :string, :enum
          "LOWER(documents.metadata->>#{qk}) #{dir} NULLS LAST"
        when :integer, :money
          "(CASE WHEN documents.metadata->>#{qk} ~ '^-?\\d{1,15}$' " \
            "THEN (documents.metadata->>#{qk})::bigint END) #{dir} NULLS LAST"
        when :number
          "(CASE WHEN documents.metadata->>#{qk} ~ '^-?\\d{1,12}(\\.\\d{1,6})?$' " \
            "THEN (documents.metadata->>#{qk})::numeric END) #{dir} NULLS LAST"
        when :date
          "(CASE WHEN documents.metadata->>#{qk} ~ '^\\d{4}-\\d{2}-\\d{2}' " \
            "THEN documents.metadata->>#{qk} END) #{dir} NULLS LAST"
        when :boolean
          "(documents.metadata->>#{qk} = 'true') #{dir} NULLS LAST"
        else
          "LOWER(documents.metadata->>#{qk}) #{dir} NULLS LAST"
        end
      end

      # Returns +scope+ with a WHERE predicate for the given operation.
      # Supported ops by type:
      #   string → :contains (ILIKE), :eq
      #   integer/money/number → :min, :max
      #   date → :from, :to
      #   enum → :in (array), :eq
      #   boolean → :eq
      # Unknown op or blank value (except :in with empty array) → scope unchanged.
      def apply_predicate(scope, op, value) # rubocop:disable Metrics/MethodLength, Metrics/CyclomaticComplexity
        if op == :in
          vals = Array(value).map(&:to_s).reject(&:blank?)
          return scope if vals.empty?
          return scope.where("documents.metadata->>? IN (?)", key, vals)
        end

        return scope if value.blank?

        case type
        when :string
          apply_string_predicate(scope, op, value)
        when :integer, :money
          apply_numeric_predicate(scope, op, value,
                                  cast: "bigint",
                                  regex: "^-?\\d{1,15}$")
        when :number
          apply_numeric_predicate(scope, op, value,
                                  cast: "numeric",
                                  regex: "^-?\\d{1,12}(\\.\\d{1,6})?$")
        when :date
          apply_date_predicate(scope, op, value)
        when :enum
          case op
          when :eq then scope.where("documents.metadata->>? = ?", key, value.to_s)
          else scope
          end
        when :boolean
          return scope unless op == :eq

          bool_str = value.in?([ true, "true", "1" ]) ? "true" : "false"
          scope.where("documents.metadata->>? = ?", key, bool_str)
        else
          apply_string_predicate(scope, op, value)
        end
      end

      private

      def apply_string_predicate(scope, op, value)
        case op
        when :contains
          like = "%#{ActiveRecord::Base.sanitize_sql_like(value.to_s)}%"
          scope.where("documents.metadata->>? ILIKE ?", key, like)
        when :eq
          scope.where("documents.metadata->>? = ?", key, value.to_s)
        else scope
        end
      end

      def apply_numeric_predicate(scope, op, value, cast:, regex:)
        coerced = coerce(value)
        return scope if coerced.nil?

        # The regex is passed as a bind parameter (not embedded in SQL) so that
        # any '?' characters inside the pattern are not mistaken for AR bind markers.
        case op
        when :min
          scope.where(
            "(CASE WHEN documents.metadata->>? ~ ? THEN (documents.metadata->>?)::#{cast} END) >= ?",
            key, regex, key, coerced
          )
        when :max
          scope.where(
            "(CASE WHEN documents.metadata->>? ~ ? THEN (documents.metadata->>?)::#{cast} END) <= ?",
            key, regex, key, coerced
          )
        else scope
        end
      end

      def apply_date_predicate(scope, op, value)
        coerced = coerce(value)
        return scope if coerced.nil?

        # Pass regex as bind param for consistency and to avoid '?' confusion.
        date_regex = '^\d{4}-\d{2}-\d{2}'

        case op
        when :from
          scope.where(
            "(CASE WHEN documents.metadata->>? ~ ? THEN documents.metadata->>? END) >= ?",
            key, date_regex, key, coerced
          )
        when :to
          scope.where(
            "(CASE WHEN documents.metadata->>? ~ ? THEN documents.metadata->>? END) <= ?",
            key, date_regex, key, coerced
          )
        else scope
        end
      end
    end

    # ── Schema class ─────────────────────────────────────────────────────────

    # Convenience constructor: read extraction_schema from a DocumentType record.
    # Returns an empty schema when +classification+ is nil.
    def self.for(classification)
      new(classification&.extraction_schema)
    end

    # +raw_schema+ is the raw extraction_schema JSONB value — either a Hash or nil.
    def initialize(raw_schema)
      @raw = raw_schema.is_a?(Hash) ? raw_schema : {}
    end

    # Ordered array of Field structs. Sorted by position when present, then by
    # insertion order (Ruby Hashes are ordered).
    def fields
      return @fields if defined?(@fields)

      @fields = @raw.each_with_index.map do |(key, defn), idx|
        build_field(key.to_s, defn.is_a?(Hash) ? defn : {}, default_position: idx + 1)
      end.sort_by { |f| f.position || Float::INFINITY }
    end

    def field(key)
      fields.find { |f| f.key == key.to_s }
    end

    def any?
      fields.any?
    end

    private

    def build_field(key, defn, default_position:)
      type          = normalize_type(defn["type"].to_s)
      label_key     = defn["label_key"]
      description   = defn["description"]
      position      = defn["position"]&.to_i || default_position
      enum_values   = defn["values"].is_a?(Array) ? defn["values"].map(&:to_s) : nil
      label         = resolve_label(label_key, description, key)

      Field.new(key: key, type: type, label: label, enum_values: enum_values, position: position)
    end

    TYPE_MAP = {
      "string"  => :string,
      "number"  => :number,
      "integer" => :integer,
      "date"    => :date,
      "money"   => :money,
      "enum"    => :enum,
      "boolean" => :boolean
    }.freeze

    def normalize_type(type_str)
      TYPE_MAP.fetch(type_str, :string)
    end

    def resolve_label(label_key, description, key)
      if label_key.present?
        translated = I18n.t(label_key, default: nil)
        return translated if translated.is_a?(String) && translated.present?
      end

      description.presence || key.to_s.humanize
    end
  end
end
