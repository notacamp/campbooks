# frozen_string_literal: true

class AddSelfGeneratedKindToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    # Campbooks' own outbound mail (digests, notifications, transactional) routinely
    # lands back in the connected mailbox and gets re-ingested. This marks such a
    # message at ingest so the AI pipeline can skip it (EmailProcessJob) and the
    # inbox can badge digests as digests. nil = ordinary third-party mail.
    # Nullable, no default, no index: a metadata-only add (no table rewrite/backfill).
    add_column :email_messages, :self_generated_kind, :string,
               comment: "Campbooks-generated mail re-ingested from the mailbox (e.g. 'digest'); nil for third-party mail. Skips the AI pipeline."
  end
end
