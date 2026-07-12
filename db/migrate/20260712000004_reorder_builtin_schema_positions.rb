# frozen_string_literal: true

# Re-runs the (idempotent) builtin schema backfill so existing DocumentType rows
# pick up the reordered canonical field positions — the most scannable fields
# (name, amount, dates) now come first, driving both the dynamic table columns
# and the document edit form order. User-added schema keys are preserved.
class ReorderBuiltinSchemaPositions < ActiveRecord::Migration[8.1]
  disable_ddl_transaction!

  def up
    DocumentTypes::Backfills::SchemaBackfill.run!
  end

  def down
    # Data-only, idempotent forward backfill — nothing to reverse.
  end
end
