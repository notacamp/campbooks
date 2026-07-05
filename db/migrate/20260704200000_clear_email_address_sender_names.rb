# frozen_string_literal: true

class ClearEmailAddressSenderNames < ActiveRecord::Migration[8.1]
  # Backfill: sender_name was incorrectly set to the raw email address (e.g.
  # "user@example.com") at all three email→Document creation sites.  The
  # Document#entity_display_name method tries sender_name first, so any doc
  # whose AI pass failed showed the user's own address as its title.
  #
  # Fix: clear sender_name wherever it looks like an email address.  The
  # column is intended for a human-readable name ("Acme Lda"), not an address.
  # Idempotent: safe to re-run; rows that are already nil are unaffected.
  def up
    Document.where("sender_name LIKE '%@%'").in_batches.update_all(sender_name: nil)
  end

  def down
    # Intentionally irreversible — the old values were wrong and have been deleted.
  end
end
