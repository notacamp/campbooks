# frozen_string_literal: true

# Adds per-match allocation tracking to support many-to-many partial payments.
#
# `allocated_cents` records the portion of the matched Document's total amount
# that THIS bank_transaction covers through this specific match. Existing
# confirmed/suggested matches get backfilled from the document's amount_cents so
# that old 1:1 full-settlement semantics are preserved.
class AddAllocatedCentsToTransactionMatches < ActiveRecord::Migration[8.1]
  def up
    add_column :transaction_matches, :allocated_cents, :bigint

    # Backfill: copy each document's amount_cents (stored in metadata JSONB)
    # into allocated_cents for every existing match. Documents without a
    # numeric, non-zero amount_cents are left NULL — the application falls back
    # gracefully (nil allocated_cents is treated as "unknown portion").
    execute <<~SQL
      UPDATE transaction_matches tm
      SET allocated_cents = (
        SELECT
          CASE
            WHEN d.metadata->>'amount_cents' ~ '^-?[0-9]+$'
             AND (d.metadata->>'amount_cents')::bigint <> 0
            THEN (d.metadata->>'amount_cents')::bigint
          END
        FROM documents d
        WHERE d.id = tm.document_id
      )
    SQL
  end

  def down
    remove_column :transaction_matches, :allocated_cents
  end
end
