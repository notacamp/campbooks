module Emails
  # Registers or renews Gmail users.watch push channels so mailbox changes
  # arrive in near-real-time instead of waiting for the every-minute
  # EmailScanJob poll.
  #
  # Prod-only: a public HTTPS callback host is required for Pub/Sub to POST to.
  # This job is a no-op unless Emails::GmailPush.configured? — which checks
  # that GMAIL_PUBSUB_TOPIC, GMAIL_PUBSUB_TOKEN, and a public callback host
  # are all present and DISABLE_GMAIL_PUSH is unset. Dev boxes (no APP_HOST)
  # and self-hosters without Pub/Sub credentials lose nothing — the minute
  # poll already covers freshness. Mirrors Calendars::WebhookRenewalJob's
  # prod-only gating style.
  #
  # Gmail watches expire after approximately 7 days. We renew any account
  # whose channel expires within 24 hours of now, giving four renewal
  # windows per day (6-hour cron) before a channel would lapse — a
  # comfortable margin even if one run misses.
  class WatchRenewalJob < ApplicationJob
    queue_as :default

    # Accounts whose watch expires beyond this threshold are fresh enough;
    # skip them this pass. Four renewal attempts (6h cron) remain before a
    # 7-day channel would lapse.
    RENEW_BEFORE = 24.hours

    def perform
      return unless Emails::GmailPush.configured?

      EmailAccount.active.google.find_each do |account|
        # Already has a live channel with more than RENEW_BEFORE to go — skip.
        next if account.push_watch_expires_at.present? &&
                account.push_watch_expires_at > RENEW_BEFORE.from_now

        begin
          result = account.mail_client.watch(Emails::GmailPush.topic)
          account.update_columns(push_watch_expires_at: result[:expires_at])
        rescue PermanentAuthError, AuthenticationError => e
          # Dead or temporarily-broken grant — log and continue. One broken
          # account must never halt the renewal sweep for the rest of the fleet.
          Rails.logger.warn("[Emails::WatchRenewalJob] auth error for account #{account.id}: #{e.class}: #{e.message}")
        rescue => e
          Rails.logger.error("[Emails::WatchRenewalJob] unexpected error for account #{account.id}: #{e.class}: #{e.message}")
        end
      end
    end
  end
end
