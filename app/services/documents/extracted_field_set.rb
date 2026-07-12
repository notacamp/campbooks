# frozen_string_literal: true

module Documents
  # Single source of truth for a document's "extracted data" field set — the same
  # per-type fields the detail page and Skim card render, sourced exclusively from
  # DocumentTypes::Schema (which reads document_types.extraction_schema).
  #
  # Resolution order:
  #   1. document.classification has an extraction_schema → schema-driven fields
  #      (label + type from the schema, values from metadata).
  #   2. No schema but metadata has keys → raw metadata listing (minus "title").
  #   3. Either branch falls back to the full raw metadata if all resolved values
  #      are blank (never_blank), so nothing extracted is ever silently hidden.
  #
  # Each field hash:
  #   key         — String, the metadata key
  #   label       — human label (from schema label_key, description, or humanized key)
  #   value       — typed Ruby value (via Field#read) or raw metadata string
  #   kind        — :text | :date | :money | :enum (Field#kind; always :text for raw)
  #   enum_values — Array of allowed values (enum fields) or nil
  #   store       — always :metadata (column store removed; SkimController updated separately)
  class ExtractedFieldSet
    def initialize(document)
      @document = document
    end

    def fields
      schema = DocumentTypes::Schema.for(@document.classification)
      if schema.any?
        never_blank(schema_fields(schema))
      else
        metadata_fields
      end
    end

    private

    def never_blank(resolved)
      return resolved if resolved.any? { |f| f[:value].to_s.strip.present? }

      metadata_fields.presence || resolved
    end

    def metadata
      @metadata ||= (@document.metadata.presence || {})
    end

    # Schema-driven: canonical labels + order from extraction_schema, values from
    # metadata. All edits land in metadata (store: :metadata).
    def schema_fields(schema)
      schema.fields.map do |field|
        {
          key:         field.key,
          label:       field.label,
          value:       field.read(metadata),
          kind:        field.kind,
          enum_values: field.enum_values,
          store:       :metadata
        }
      end
    end

    # Unknown / unclassified type: surface whatever the AI wrote into metadata so
    # nothing extracted is ever hidden.
    def metadata_fields
      metadata.except("title").map do |name, value|
        { key: name.to_s, label: name.to_s.humanize, value: value,
          kind: :text, enum_values: nil, store: :metadata }
      end
    end
  end
end
