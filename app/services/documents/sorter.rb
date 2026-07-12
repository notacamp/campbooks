# frozen_string_literal: true

module Documents
  # Resolves sort params into a safe ORDER BY fragment for the Documents list.
  #
  # Usage:
  #   sorter = Documents::Sorter.from_params(params, document_type: dt)
  #   sorter.active?      # => true/false
  #   sorter.apply(scope) # => scope with ORDER BY
  #   sorter.to_h         # => { "sort" => "amount_cents", "dir" => "desc" }
  #
  # Universal sort keys (always allowed):
  #   "added" -> documents.created_at
  #   "name"  -> SQL approximation of display_title (title -> vendor/client -> filename)
  #
  # Extracted-field keys: only when +document_type+ (a DocumentType record) is given
  # AND the key exists in DocumentTypes::Schema.for(document_type). Unknown or
  # disallowed keys produce an inactive sorter (caller falls back to default order).
  class Sorter
    UNIVERSAL_KEYS = %w[added name].freeze

    attr_reader :key, :dir

    # Build from request params.
    # +document_type+ must be the single DocumentType record when the active filters
    # narrow to exactly one type; pass nil otherwise (extracted-field keys are rejected).
    def self.from_params(params, document_type: nil)
      p = if params.respond_to?(:to_unsafe_h)
        params.to_unsafe_h.with_indifferent_access
      else
        (params || {}).with_indifferent_access
      end
      new(p[:sort].to_s.strip, p[:dir].to_s.strip, document_type: document_type)
    end

    def initialize(sort_key, dir_param, document_type: nil) # rubocop:disable Metrics/MethodLength
      @dir = dir_param.to_s.strip.downcase == "desc" ? "desc" : "asc"
      @key, @order_fragment = resolve(sort_key.to_s.strip, document_type)
    end

    # True when a valid, allowed sort key was resolved.
    def active?
      @order_fragment.present?
    end

    # Apply the ORDER BY to +scope+. Always appends a stable tiebreak
    # (created_at DESC, id DESC) after the primary column so identical values
    # produce a repeatable page sequence.
    def apply(scope)
      return scope unless active?

      scope.reorder(Arel.sql(@order_fragment))
           .order(created_at: :desc, id: :desc)
    end

    # URL round-trip hash. Empty when the sorter is inactive.
    def to_h
      return {} unless active?

      { "sort" => @key, "dir" => @dir }
    end

    private

    # Resolves +sort_key+ to [ key, sql_fragment ] or [ nil, nil ].
    def resolve(sort_key, document_type) # rubocop:disable Metrics/MethodLength
      return [ nil, nil ] if sort_key.blank?

      case sort_key
      when "added"
        [ sort_key, "documents.created_at #{@dir.upcase} NULLS LAST" ]
      when "name"
        # SQL approximation of Document#display_title:
        # metadata["title"] -> entity display name -> canonical_filename
        sql = "LOWER(COALESCE(" \
              "NULLIF(documents.metadata->>'title', ''), " \
              "NULLIF(documents.metadata->>'vendor_name', ''), " \
              "NULLIF(documents.metadata->>'client_name', ''), " \
              "documents.canonical_filename)) #{@dir.upcase} NULLS LAST"
        [ sort_key, sql ]
      else
        resolve_field_key(sort_key, document_type)
      end
    end

    # Looks the key up in the single-selected DocumentType's schema.
    # Returns [ nil, nil ] for unknown keys or when no type is pinned.
    def resolve_field_key(sort_key, document_type)
      return [ nil, nil ] if document_type.nil?

      schema = DocumentTypes::Schema.for(document_type)
      field  = schema.field(sort_key)
      return [ nil, nil ] if field.nil?

      [ sort_key, field.order_sql(@dir.to_sym) ]
    end
  end
end
