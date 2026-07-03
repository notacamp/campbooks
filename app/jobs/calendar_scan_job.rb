class CalendarScanJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3
  discard_on AuthenticationError # Permanent auth failures — wait for next scheduled run

  # scope: "incremental" (default, every-minute poll) pulls only changes since the
  # stored per-calendar sync token — fast. "full" refreshes the calendar list and
  # re-pulls the whole event window; used by the 15-minute sweep, on account link,
  # and as the HTTP-410 sync-token recovery path. Mirrors EmailScanJob#perform.
  def perform(calendar_account_id = nil, scope = "incremental")
    accounts = if calendar_account_id
      CalendarAccount.where(id: calendar_account_id).active
    else
      CalendarAccount.active
    end

    reconcile_stale_scans(accounts)

    accounts.each { |account| sync_account(account, scope) }
  end

  private

  # Clear claims stranded by a worker killed mid-sync, so the slot frees up and
  # (once the pill exists) the live indicator can't stick on. Fresh claims are
  # left alone.
  def reconcile_stale_scans(accounts)
    accounts.where(scanning: true)
            .where("scan_started_at IS NULL OR scan_started_at < ?", CalendarAccount::SCAN_STALE_AFTER.ago)
            .update_all(scanning: false)
  end

  def sync_account(account, scope)
    unless claim_scan_slot(account)
      Rails.logger.info("[CalendarScanJob] Skipping #{account.email_address} — sync already in progress")
      return
    end

    sync_log = nil
    begin
      sync_log = account.calendar_sync_logs.create!(status: :running, started_at: Time.current)
      client = account.calendar_client

      clear_stale_pending(account)

      # Refresh the calendar list on a full sweep, and whenever we still have no
      # calendars — so an account self-heals on the next minute-scan once the
      # provider API is enabled/authorized after a first failed discovery.
      refresh_calendar_list(account, client) if scope == "full" || account.calendars.empty?

      found = 0
      upserted = 0
      account.calendars.syncing.find_each do |calendar|
        result = sync_calendar(client, calendar, scope)
        found += result[:found]
        upserted += result[:upserted]
      end

      sync_log.update!(status: :completed, completed_at: Time.current,
                       events_found: found, events_upserted: upserted)
    rescue => e
      sync_log&.update!(status: :failed, completed_at: Time.current,
                        error_messages: [ { error: e.message } ])
      raise
    ensure
      release_scan_slot(account)
    end
  end

  def refresh_calendar_list(account, client)
    client.calendar_list.each do |attrs|
      calendar = account.calendars.find_or_initialize_by(provider_calendar_id: attrs[:provider_calendar_id])
      is_new = calendar.new_record?
      calendar.assign_attributes(
        name: attrs[:name], description: attrs[:description],
        time_zone: attrs[:time_zone], is_primary: attrs[:is_primary], is_writable: attrs[:is_writable]
      )
      # Seed the provider's color only on first discovery — the calendar's color
      # is user-editable (sidebar picker) and must survive full sweeps.
      calendar.color = attrs[:color] if is_new
      # Auto-enable the primary calendar the first time we see it; never override a
      # user's later on/off choice.
      calendar.syncing = true if is_new && attrs[:is_primary]
      calendar.save!
    end
  rescue AuthenticationError
    raise
  rescue => e
    Rails.logger.error("[CalendarScanJob] calendar_list refresh failed for #{account.email_address}: #{e.message}")
  end

  def sync_calendar(client, calendar, scope)
    result =
      if scope == "full" || calendar.sync_token.blank?
        pull_full(client, calendar)
      else
        begin
          client.list_events_incremental(calendar)
        rescue Calendars::SyncTokenExpired
          # Token invalid: don't hammer the API now — schedule a jittered, rate-
          # limited full resync (Risk 2) and skip this calendar this pass.
          Calendars::FullResyncJob.set(wait: rand(0..300).seconds).perform_later(calendar.id)
          return { found: 0, upserted: 0 }
        end
      end

    upserted = upsert_events(calendar, result[:events])
    stamp_calendar(calendar, result[:next_sync_token])
    { found: result[:events].size, upserted: upserted }
  end

  def pull_full(client, calendar)
    window_start = 90.days.ago
    window_end = 365.days.from_now
    calendar.update_columns(sync_window_start: window_start, sync_window_end: window_end)
    client.list_events_full(calendar, time_min: window_start, time_max: window_end)
  end

  def stamp_calendar(calendar, next_sync_token)
    cols = { last_event_sync_at: Time.current }
    cols[:sync_token] = next_sync_token if next_sync_token.present?
    calendar.update_columns(cols)
  end

  def upsert_events(calendar, events)
    count = 0
    events.each do |attrs|
      pid = attrs[:provider_event_id]
      next if pid.blank?

      event = calendar.calendar_events.find_or_initialize_by(provider_event_id: pid)

      if event.persisted?
        # Loop-avoidance (Risk 1): skip our own in-flight writes and unchanged rows.
        next if event.outbound_pending?
        next if attrs[:provider_etag].present? && attrs[:provider_etag] == event.provider_etag
      end

      if attrs[:status] == "cancelled"
        # Tombstone in place — keep the row's data, just mark it cancelled. Ignore
        # cancellations of events we never stored.
        next unless event.persisted?
        event.update_columns(status: CalendarEvent.statuses[:cancelled],
                             provider_etag: attrs[:provider_etag], updated_at: Time.current)
        count += 1
        next
      end

      next if attrs[:start_at].blank? # skip malformed (non-cancelled) entries

      event.assign_attributes(event_attributes(attrs))
      event.save!
      count += 1
    end
    count
  end

  def event_attributes(attrs)
    {
      title: attrs[:title],
      description: attrs[:description],
      location: attrs[:location],
      html_link: attrs[:html_link],
      conference_url: attrs[:conference_url],
      start_at: attrs[:start_at],
      end_at: attrs[:end_at],
      start_time_zone: attrs[:start_time_zone],
      end_time_zone: attrs[:end_time_zone],
      all_day: attrs[:all_day] || false,
      status: attrs[:status].to_s.presence_in(%w[confirmed tentative cancelled]) || "confirmed",
      rsvp_status: attrs[:rsvp_status],
      is_organizer: attrs[:is_organizer] || false,
      attendees: attrs[:attendees] || [],
      provider_etag: attrs[:provider_etag],
      provider_sequence: attrs[:provider_sequence],
      rrule: attrs[:rrule],
      recurring_event_provider_id: attrs[:recurring_event_provider_id],
      original_start_at: attrs[:original_start_at]
    }
  end

  # A write job that failed permanently leaves outbound_pending: true, which the
  # upsert then skips forever. Clear the flag once it's clearly stale so inbound
  # sync can reconcile the row (plan Risk 1 hardening).
  def clear_stale_pending(account)
    CalendarEvent.joins(:calendar)
                 .where(calendars: { calendar_account_id: account.id })
                 .where(outbound_pending: true)
                 .where(calendar_events: { updated_at: ..10.minutes.ago })
                 .update_all(outbound_pending: false)
  end

  def claim_scan_slot(account)
    claimed = CalendarAccount.where(id: account.id)
                             .where("scanning = false OR scan_started_at < ?", CalendarAccount::SCAN_STALE_AFTER.ago)
                             .update_all(scanning: true, scan_started_at: Time.current) == 1
    if claimed
      account.calendar_sync_logs.where(status: :running)
             .update_all(status: CalendarSyncLog.statuses[:failed], completed_at: Time.current)
    end
    claimed
  end

  def release_scan_slot(account)
    account.update_columns(scanning: false, last_scanned_at: Time.current)
  end
end
