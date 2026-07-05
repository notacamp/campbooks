module Calendars
  # Recovery path when a Google sync token expires (HTTP 410). Rate-limited to one
  # resync per calendar per 6 hours via Rails.cache so a wave of simultaneous
  # token expiries can't trigger a thundering herd of full re-fetches (Risk 2).
  class FullResyncJob < ApplicationJob
    queue_as :default

    RATE_LIMIT = 6.hours

    def perform(calendar_id)
      calendar = Calendar.find_by(id: calendar_id)
      return unless calendar&.calendar_account&.active?

      key = "calendar_full_resync/#{calendar_id}"
      return if Rails.cache.read(key)
      Rails.cache.write(key, true, expires_in: RATE_LIMIT)

      # Discard the stale token so the next sweep does a full pull for this calendar.
      Current.set(workspace: calendar.workspace) do
        calendar.update_columns(sync_token: nil)
        CalendarScanJob.perform_later(calendar.calendar_account_id, "full")
      end
    end
  end
end
