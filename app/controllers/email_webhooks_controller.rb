# Public receiver for Gmail Pub/Sub push notifications. Google POSTs here when
# a watched mailbox changes, via a push subscription the operator configures in
# the Google Cloud console. We verify the shared token in the query string,
# decode the Pub/Sub envelope, find the account, debounce burst pings, then
# enqueue a cheap delta scan for that account.
#
# Answering 204 (any 2xx) ACKs the Pub/Sub message. A non-2xx causes Pub/Sub
# to retry with exponential back-off — correct for real transient failures, but
# a poison-message loop for malformed bodies we can never parse. Those get a
# 204 so they are ACKed and discarded rather than retried forever.
#
# Unauthenticated by design — the shared token in ?token= is the secret.
# Mirrors CalendarWebhooksController's public setup.
class EmailWebhooksController < ApplicationController
  allow_unauthenticated_access
  skip_before_action :ensure_workspace
  skip_before_action :redirect_to_onboarding_if_incomplete
  skip_forgery_protection
  # Rate-limit per source IP. Pub/Sub delivers from a small fixed set of
  # Google IPs, so this is coarse, but it caps runaway replay floods.
  rate_limit to: 120, within: 1.minute, only: :gmail_receive,
             by: -> { request.remote_ip },
             with: -> { head :too_many_requests }

  def gmail_receive
    unless Emails::GmailPush.configured? && Emails::GmailPush.valid_token?(params[:token])
      head :not_found
      return
    end

    email_address = parse_email_address
    # Malformed body returns nil from parse_email_address (already rescued).
    # ACK with 204 so Pub/Sub does not retry an unprocessable message forever.
    return head :no_content if email_address.blank?

    account = EmailAccount.active.google.find_by("lower(email_address) = ?", email_address)
    # Unknown address (account disconnected, wrong project, etc.) — ACK silently.
    return head :no_content unless account

    # Debounce: Gmail emits a burst of pings during bulk operations (mass read,
    # import, flag sweeps). The delta scan is slot-locked inside EmailScanJob,
    # so concurrent enqueues are safe, but we skip needless job-queue noise by
    # only enqueueing once per debounce window.
    cache_key = "gmail_push_debounce/#{account.id}"
    wrote = Rails.cache.write(cache_key, true, expires_in: 10.seconds, unless_exist: true)
    EmailScanJob.perform_later(account.id, "delta") if wrote

    head :no_content
  end

  private

  # Decodes the Pub/Sub push envelope and extracts the email address.
  # The envelope is { "message": { "data": <base64 JSON>, ... } } where the
  # inner JSON is { "emailAddress": "...", "historyId": ... }.
  # Returns nil (not raises) on any parse failure so the controller can ACK
  # the poison message instead of returning 5xx and triggering a retry loop.
  def parse_email_address
    envelope = JSON.parse(request.raw_post)
    inner    = JSON.parse(Base64.decode64(envelope.dig("message", "data").to_s))
    inner["emailAddress"].to_s.downcase.presence
  rescue JSON::ParserError, ArgumentError, TypeError
    # TypeError: a valid-JSON-but-wrong-shape body (e.g. a bare array) makes
    # dig raise; that is a poison message too, not a server error.
    nil
  end
end
