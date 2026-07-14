# frozen_string_literal: true

# Messages synced from Zoho before Zoho::MailClient decoded its HTML-escaped
# metadata are stored as "&lt;user@example.com&gt;" — which broke reply-all
# self-exclusion (users emailed themselves) and address display. One idempotent
# background job repairs the stored rows, kept OFF the boot / db:prepare
# critical path (self-hosted upgrades must be one step, and boot must not wait
# on a data sweep).
#
# Best-effort and non-fatal: new mail is stored decoded regardless, and the
# composer decodes legacy rows defensively — a failed enqueue (e.g. the queue
# isn't reachable during boot) must not break the migration/deploy.
class EnqueueZohoEscapedFieldsBackfill < ActiveRecord::Migration[8.1]
  def up
    Emails::ZohoEscapedFieldsBackfillJob.perform_later
  rescue StandardError => e
    say "Skipped Zoho escaped-fields backfill enqueue (#{e.class}: #{e.message}); run Emails::ZohoEscapedFieldsBackfillJob manually."
  end

  def down
    # No-op: decoding is a one-way data repair.
  end
end
