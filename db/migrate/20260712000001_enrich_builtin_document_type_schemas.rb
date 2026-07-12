# frozen_string_literal: true

class EnrichBuiltinDocumentTypeSchemas < ActiveRecord::Migration[8.1]
  # Data-only migration: no structural changes.
  # Safe to skip (self-hosters running older versions catch up via db:prepare).
  disable_ddl_transaction!

  def up
    DocumentTypes::Backfills::SchemaBackfill.run!
  end

  def down
    # Irreversible — the pre-merge schemas are not stored anywhere.
    # Re-running `up` is safe (idempotent).
  end
end
