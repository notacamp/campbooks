class EmailScanJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3
  discard_on AuthenticationError  # Permanent auth failures — don't retry, wait for next scheduled run

  # The provider-agnostic sync engine. It owns the slot-lock, the scan log, and the
  # live "syncing" pill; the actual fetch is delegated to the account's per-vendor
  # Emails::SyncStrategies strategy (Gmail history delta, Graph per-folder delta,
  # Zoho windowed). Modes:
  #
  #   "delta" (default, every-minute poll) — strategy#sync!, the cheap incremental
  #     pull across every folder. A never-baselined account (or one whose cursor
  #     expired mid-pull) hands off to Emails::FullResyncJob instead of walking
  #     inline, so the minute poll stays fast.
  #   "full" (one account) — strategy#full_resync!, the heavy walk that bootstraps
  #     or recovers the cursor. Enqueued by Emails::FullResyncJob.
  #   "resync_sweep" (every 15 min) — enqueue a "full" run for each vendor that has
  #     no change feed (Zoho), so read/flag drift on existing mail is reconciled.
  #     Delta vendors skip it; their feed already carries those changes.
  def perform(email_account_id = nil, mode = "delta")
    accounts = if email_account_id
      EmailAccount.where(id: email_account_id).active
    else
      EmailAccount.active
    end

    reconcile_stale_scans(accounts)

    case mode
    when "resync_sweep"
      accounts.find_each do |account|
        EmailScanJob.perform_later(account.id, "full") if account.sync_strategy.needs_periodic_resync?
      end
    when "full"
      accounts.each { |account| scan_account(account, full: true) }
    else
      accounts.each { |account| scan_account(account, full: false) }
    end
  end

  private

  # A worker killed mid-scan (a hard process exit) skips the `ensure` in
  # #scan_account and strands `scanning = true`. The poll would re-claim the slot
  # once it goes stale, but until that rescan completes — and never, if it dies
  # too — no broadcast turns the live "syncing" pill off, so it sticks on screen.
  # So before scanning, proactively clear any claim that has gone stale and push
  # the corrected status to each affected user. Fresh claims (a scan genuinely in
  # flight) are left untouched.
  def reconcile_stale_scans(accounts)
    accounts.where(scanning: true)
            .where("scan_started_at IS NULL OR scan_started_at < ?", EmailAccount::SCAN_STALE_AFTER.ago)
            .find_each do |account|
      account.update_columns(scanning: false)
      broadcast_sync_status(account)
    end
  end

  # Run one account's sync. `full: false` is the cheap delta pull; `full: true` is
  # the heavy re-baseline. Everything past the slot claim runs inside begin/ensure
  # so the slot is *always* released and the pill *always* re-broadcast — even if a
  # provider call raises. A leaked `scanning = true` is exactly what strands the
  # live pill on "syncing" (the next poll then skips the still-claimed account, so
  # nothing ever broadcasts it off).
  def scan_account(account, full: false)
    strategy = account.sync_strategy

    # Never baselined → offload the heavy first walk to the (rate-limited)
    # FullResyncJob rather than block the minute poll on it. The slot stays free,
    # so we don't strand the pill while the resync is queued.
    if !full && strategy.needs_bootstrap?
      Emails::FullResyncJob.perform_later(account.id)
      return
    end

    unless claim_scan_slot(account)
      Rails.logger.info("[EmailScanJob] Skipping account #{account.email_address} — scan already in progress")
      return
    end

    scan_log = nil
    begin
      broadcast_sync_status(account) # scanning just turned on — show the sync pill

      scan_log = account.email_scan_logs.create!(status: :running, started_at: Time.current)
      result = full ? strategy.full_resync!(scan_log: scan_log) : strategy.sync!(scan_log: scan_log)

      scan_log.update!(
        status: :completed,
        completed_at: Time.current,
        emails_found: result.found,
        emails_processed: result.created
      )
    rescue Emails::CursorExpired
      # The delta cursor died (Gmail historyId aged out, Graph deltaToken expired)
      # — recover with a full re-baseline, offloaded and rate-limited. Not a
      # failure: close the log cleanly so the sync dashboard reads true.
      scan_log&.update!(status: :completed, completed_at: Time.current)
      Emails::FullResyncJob.perform_later(account.id)
    rescue => e
      scan_log&.update!(
        status: :failed,
        completed_at: Time.current,
        error_messages: [ { error: e.message } ]
      )
      raise
    ensure
      release_scan_slot(account)
      broadcast_sync_status(account) # scanning finished — clear the sync pill
    end
  end

  def claim_scan_slot(account)
    claimed = EmailAccount.where(id: account.id)
                          .where("scanning = false OR scan_started_at < ?", EmailAccount::SCAN_STALE_AFTER.ago)
                          .update_all(scanning: true, scan_started_at: Time.current) == 1
    if claimed
      # Fail any logs orphaned by a worker killed mid-scan so the sync dashboard
      # doesn't show them as perpetually "running".
      account.email_scan_logs.where(status: :running)
             .update_all(status: EmailScanLog.statuses[:failed], completed_at: Time.current)
    end
    claimed
  end

  def release_scan_slot(account)
    account.update_columns(scanning: false, last_scanned_at: Time.current)
  end

  # New email arriving is not notification-worthy (the inbox is that surface).
  # Instead we surface a live bottom-center "syncing" pill while a scan runs,
  # rendered per-user from their own current scanning state.
  def broadcast_sync_status(account)
    account.users.find_each do |user|
      # Render the pill through a real view context, in the user's locale. A bare
      # `.call` renders the Phlex component with no view context, so its `t(...)`
      # raises `undefined method 't' for nil` and takes the whole scan down before
      # any mail is fetched. `layout: false` keeps the request-only application
      # layout (CSRF/CSP meta tags) out of this job-side render.
      html = I18n.with_locale(user.locale.presence || I18n.default_locale) do
        ApplicationController.render(
          Campbooks::SyncIndicator.new(scanning: user.email_syncing?),
          layout: false
        )
      end
      Turbo::StreamsChannel.broadcast_replace_to(
        "sync_status_#{user.id}",
        target: Campbooks::SyncIndicator::DOM_ID,
        html: html
      )
      # Keep the Skim tray live as a scan ingests new mail.
      Emails::SkimTrayBroadcaster.refresh(user)
    end
  end
end
