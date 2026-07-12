# frozen_string_literal: true

module DocumentTypes
  module Backfills
    # Migration 1 helper: merges DocumentTypes::BuiltinSchemas canonical entries
    # INTO existing DocumentType rows for the five built-in names.
    #
    # Merge rules:
    #   - Canonical entries WIN for any key present in BuiltinSchemas.
    #   - Extra user-added keys (absent from the canonical schema) are preserved
    #     in the same definition, but receive a "position" after the canonical block
    #     when they lack one.
    #   - NULL or empty existing schema → replaced entirely by the canonical schema.
    #   - Custom-named document types (not in BUILTIN_NAMES) are untouched.
    #   - Idempotent: re-running produces identical rows.
    class SchemaBackfill
      BUILTIN_NAMES = DocumentTypes::BuiltinSchemas::ALL.keys.freeze

      # Stub model that sees all columns without any app-level ignored_columns magic.
      class MigDocumentType < ActiveRecord::Base
        self.table_name = "document_types"
      end

      def self.run!
        new.run!
      end

      def run!
        conn = ActiveRecord::Base.connection

        MigDocumentType.where(name: BUILTIN_NAMES).find_each do |dt|
          canonical = DocumentTypes::BuiltinSchemas.for(dt.name)
          next unless canonical

          merged = merge_schema(dt.extraction_schema, canonical)

          # Skip the UPDATE when the schema is already correct (idempotent fast path).
          next if merged == dt.extraction_schema

          conn.execute(
            ActiveRecord::Base.sanitize_sql_array([
              "UPDATE document_types SET extraction_schema = ?::jsonb, updated_at = NOW() WHERE id = ?",
              merged.to_json,
              dt.id.to_s
            ])
          )
        end
      end

      private

      def merge_schema(existing, canonical)
        existing = existing.is_a?(Hash) ? existing : {}

        # Max position used by canonical entries (1-based).
        max_canonical_pos = canonical.values.filter_map { |v| v["position"] }.max || 0
        next_pos = max_canonical_pos + 1

        # Preserve user-added keys (absent from canonical), assigning positions
        # after the canonical block when the definition lacks one.
        user_additions = {}
        existing.each do |key, defn|
          next if canonical.key?(key)

          defn = defn.is_a?(Hash) ? defn.dup : {}
          defn["position"] ||= next_pos
          next_pos += 1
          user_additions[key] = defn
        end

        # Canonical entries WIN for overlapping keys; user additions appended after.
        canonical.merge(user_additions)
      end
    end
  end
end
