module Emails
  # Recovery/bootstrap path for a delta-synced mailbox: triggers a full re-baseline
  # (walk every folder, ingest, (re)establish the cursor) by handing off to
  # EmailScanJob in "full" mode — which owns the slot-lock, scan log, and live sync
  # pill, so the heavy walk shows up like any other scan.
  #
  # Enqueued when an account has never been baselined (bootstrap) or when its delta
  # cursor expires (Gmail historyId 404 / Graph deltaToken 410). Rate-limited to one
  # resync per account per hour via Rails.cache, so a wave of expiries — or the
  # minute poll re-enqueuing every cycle while a bootstrap is still running — can't
  # stampede the provider. Mirrors Calendars::FullResyncJob.
  #
  # The periodic Zoho read/flag reconcile does NOT come through here: it's scheduled,
  # not storm-triggered, so EmailScanJob enqueues it directly (see "resync_sweep").
  class FullResyncJob < ApplicationJob
    queue_as :default
    discard_on ActiveJob::DeserializationError

    RATE_LIMIT = 1.hour

    def perform(account_id)
      account = EmailAccount.find_by(id: account_id)
      return unless account&.active?

      key = "email_full_resync/#{account_id}"
      return if Rails.cache.read(key)

      Rails.cache.write(key, true, expires_in: RATE_LIMIT)
      EmailScanJob.perform_later(account_id, "full")
    end
  end
end
