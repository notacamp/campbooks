# Public receiver for Google Calendar `events.watch` push notifications. Google
# POSTs here when a watched calendar changes; we verify the per-channel token we
# set at registration, then enqueue an incremental sync for that account. The
# body is a change ping (no event data), so we just trigger the normal pull and
# answer 200 immediately. Unauthenticated by design — the channel token is the
# shared secret. Mirrors WebhooksController's public setup.
class CalendarWebhooksController < ApplicationController
  allow_unauthenticated_access
  skip_before_action :ensure_workspace
  skip_before_action :redirect_to_onboarding_if_incomplete
  skip_forgery_protection
  # Throttle this public push endpoint so a flood of (forged or replayed) pings
  # can't fan out into a storm of CalendarScanJobs / Google API calls. Keyed per
  # channel id, falling back to IP.
  rate_limit to: 60, within: 1.minute, only: :google_receive,
             by: -> { request.headers["X-Goog-Channel-Id"].presence || request.remote_ip },
             with: -> { head :too_many_requests }

  def google_receive
    channel = CalendarWebhookChannel.find_by(provider_channel_id: request.headers["X-Goog-Channel-Id"])
    if channel && token_matches?(channel)
      account = channel.calendar.calendar_account
      CalendarScanJob.perform_later(account.id, "incremental") if account&.active?
    end
    head :ok
  end

  private

  def token_matches?(channel)
    ActiveSupport::SecurityUtils.secure_compare(
      channel.channel_token.to_s,
      request.headers["X-Goog-Channel-Token"].to_s
    )
  end
end
