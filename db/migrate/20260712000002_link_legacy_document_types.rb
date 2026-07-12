# frozen_string_literal: true

class LinkLegacyDocumentTypes < ActiveRecord::Migration[8.1]
  # Data-only migration: sets document_type_id from the legacy integer enum.
  # Batched; safe to run on large tables via db:prepare on boot.
  disable_ddl_transaction!

  def up
    DocumentTypes::Backfills::ClassificationLinkBackfill.run!
  end

  def down
    # Irreversible: cannot distinguish ids set by this migration vs pre-existing ones.
  end
end
