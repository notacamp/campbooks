# frozen_string_literal: true

class BackfillMetadataFromLegacyColumns < ActiveRecord::Migration[8.1]
  # Data-only migration: copies non-NULL legacy column values into metadata JSONB.
  # Column values win over existing metadata. Batched; idempotent.
  disable_ddl_transaction!

  def up
    DocumentTypes::Backfills::MetadataValueBackfill.run!
  end

  def down
    # Irreversible data migration.
  end
end
