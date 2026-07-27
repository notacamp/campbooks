module Emails
  # Configuration and token validation for Gmail real-time push (users.watch
  # via Cloud Pub/Sub). This module is the single gating point for the feature
  # — every path that touches Gmail push checks configured?/valid_token? here.
  #
  # Operator set-up:
  #   GMAIL_PUBSUB_TOPIC  — full Pub/Sub topic name ("projects/.../topics/...")
  #   GMAIL_PUBSUB_TOKEN  — shared secret appended to the push-subscription URL
  #                         as ?token=<value>; Google sends it back on every POST
  #   DISABLE_GMAIL_PUSH=1 — force-disables push even when the above are set
  #
  # Without GMAIL_PUBSUB_TOPIC + GMAIL_PUBSUB_TOKEN, or without a public
  # callback host (dev boxes, self-hosters without APP_HOST), Gmail stays on
  # the every-minute EmailScanJob poll — no degradation for the end user.
  #
  # Host detection mirrors Calendars::WebhookRenewalJob#callback_url verbatim
  # (intentional duplication — see that class for the rationale).
  module GmailPush
    class << self
      def topic
        ENV["GMAIL_PUBSUB_TOPIC"].presence
      end

      def token
        ENV["GMAIL_PUBSUB_TOKEN"].presence
      end

      # Push is active only when the topic and token ENV vars are set,
      # DISABLE_GMAIL_PUSH is not "1", and a public callback host is present.
      # Dev boxes (no APP_HOST, no mailer host) return false so the minute
      # poll covers freshness without any operator action.
      def configured?
        topic.present? &&
          token.present? &&
          !ActiveModel::Type::Boolean.new.cast(ENV["DISABLE_GMAIL_PUSH"]) &&
          public_callback_host?
      end

      # Constant-time comparison so timing side-channels cannot leak the token.
      # Returns false for nil or blank input on either side.
      def valid_token?(candidate)
        return false if token.blank? || candidate.blank?

        ActiveSupport::SecurityUtils.secure_compare(token, candidate.to_s)
      end

      private

      # Mirror of Calendars::WebhookRenewalJob#callback_url host-detection:
      # returns true when APP_HOST or the action_mailer default host is set.
      def public_callback_host?
        host = ENV["APP_HOST"].presence || mailer_host
        host.present?
      end

      def mailer_host
        (Rails.application.config.action_mailer.default_url_options || {})[:host]
      end
    end
  end
end
