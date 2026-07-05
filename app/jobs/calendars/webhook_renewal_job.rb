module Calendars
  # Registers and renews Google `events.watch` push channels for syncing
  # calendars, so changes arrive in near-real-time instead of waiting for the
  # minute poll. Runs daily from config/recurring.yml (production).
  #
  # Prod-only: `events.watch` needs a public HTTPS callback. This is a no-op when
  # DISABLE_CALENDAR_PUSH is set or no callback host is configured (dev), where
  # the every-minute CalendarScanJob already covers freshness. Zoho is not
  # supported here (polling only).
  class WebhookRenewalJob < ApplicationJob
    queue_as :default
    RENEW_BEFORE = 24.hours
    CHANNEL_TTL = 7.days

    def perform
      return if push_disabled?
      address = callback_url
      return if address.blank?

      Calendar.syncing.includes(:calendar_account, :calendar_webhook_channels).find_each do |calendar|
        account = calendar.calendar_account
        next unless account.active? && account.google?

        current = calendar.calendar_webhook_channels.max_by { |c| c.expires_at || Time.at(0) }
        next if current&.expires_at && current.expires_at > RENEW_BEFORE.from_now

        Current.set(workspace: calendar.workspace) { register(calendar, address, replacing: current) }
      end
    end

    private

    def register(calendar, address, replacing: nil)
      client = calendar.calendar_account.calendar_client
      channel_id = "cb-#{calendar.id}-#{SecureRandom.hex(6)}"
      token = SecureRandom.hex(24)

      res = client.watch_calendar(calendar, channel_id: channel_id, token: token, address: address, ttl_seconds: CHANNEL_TTL.to_i)

      calendar.calendar_webhook_channels.create!(
        provider_channel_id: channel_id,
        provider_resource_id: res["resourceId"],
        channel_token: token,
        expires_at: (res["expiration"].present? ? Time.at(res["expiration"].to_i / 1000) : CHANNEL_TTL.from_now)
      )

      if replacing
        client.stop_channel(channel_id: replacing.provider_channel_id, resource_id: replacing.provider_resource_id) rescue nil
        replacing.destroy
      end
    rescue => e
      Rails.logger.error("[Calendars::WebhookRenewalJob] register failed for calendar #{calendar.id}: #{e.class}: #{e.message}")
    end

    def push_disabled?
      ActiveModel::Type::Boolean.new.cast(ENV["DISABLE_CALENDAR_PUSH"])
    end

    # The public callback URL Google will POST change pings to. Sourced from
    # APP_HOST or the mailer host; nil (⇒ no-op) when neither is set.
    def callback_url
      host = ENV["APP_HOST"].presence || mailer_host
      return nil if host.blank?
      scheme = host.start_with?("localhost", "127.0.0.1") ? "http" : "https"
      "#{scheme}://#{host}/calendar_webhooks/google"
    end

    def mailer_host
      (Rails.application.config.action_mailer.default_url_options || {})[:host]
    end
  end
end
