# frozen_string_literal: true

module Emails
  # Recognises Campbooks' OWN outbound mail — digests, notifications, transactional
  # — when it lands back in a connected mailbox, so the ingest pipeline can skip the
  # AI extractors (and flag digests for the reader) instead of mining our own mail
  # for the reminders / tasks / contacts it already lists.
  #
  # Detection needs TWO signals, because MAILER_FROM can be a *shared* sending
  # identity: one no-reply@ address is often used by the app AND by unrelated
  # services on the same domain (health-monitoring alerts, a status page). Matching
  # on the From address alone swallows those third-party alerts and robs them of
  # triage and inbox rules. So a message is ours only when BOTH hold:
  #
  #   1. From is our own mailer address (MAILER_FROM). SPF/DMARC-protected, so a
  #      third party can't spoof it.
  #   2. It carries our X-Campbooks-Kind marker — stamped on ALL app mail by
  #      ApplicationMailer (refined to "digest" by DigestMailer). Gmail / Microsoft
  #      surface it on the way back in, so its ABSENCE there means a *different*
  #      sender is legitimately using our shared From address → ordinary inbound.
  #
  # Zoho strips transport headers from its list endpoint, so the marker can never
  # arrive on a Zoho-synced message; there (`headers_available: false`) we fall back
  # to the address-only signal — best-effort, at the cost of not distinguishing a
  # shared-address third party on Zoho.
  #
  # Dependency-free (sender address + provider capability are injected) so it
  # unit-tests in isolation, like Emails::Categorizer.
  class SelfGeneratedDetector
    GENERIC_KIND = "campbooks"

    # Kinds a message is allowed to declare via X-Campbooks-Kind. Anything else
    # (including a spoofed value) collapses to the generic kind.
    KNOWN_KINDS = %w[digest].freeze

    def self.kind_for(msg, mailer_from:, headers_available: true)
      new(msg, mailer_from: mailer_from, headers_available: headers_available).kind
    end

    # headers_available: does the syncing provider surface transport headers (and so
    # our X-Campbooks-Kind marker)? True for Gmail/Microsoft, false for Zoho.
    def initialize(msg, mailer_from:, headers_available: true)
      @msg = msg || {}
      @mailer_from = mailer_from
      @headers_available = headers_available
    end

    # The self-generated kind ("digest" or "campbooks"), or nil for third-party mail.
    def kind
      return nil unless from_our_mailer?

      declared = header("header_campbooks_kind")
      return KNOWN_KINDS.include?(declared) ? declared : GENERIC_KIND if declared

      # From our shared address but WITHOUT our marker. On a provider that surfaces
      # headers (Gmail/Microsoft) our own mail always carries it, so its absence
      # means a different sender is using the address — not ours. Only when headers
      # are unavailable (Zoho) do we fall back to the address-only signal.
      @headers_available ? nil : GENERIC_KIND
    end

    private

    # nil for a blank header; plain Ruby (no ActiveSupport) so the class stays
    # unit-testable in isolation like Emails::Categorizer.
    def header(key)
      value = @msg[key].to_s.strip.downcase
      value.empty? ? nil : value
    end

    # True when the message's From is our own mailer address. Compares bare,
    # down-cased addresses so "Campbooks <no-reply@x>" and "no-reply@x" match.
    def from_our_mailer?
      ours = address_in(@mailer_from)
      !ours.nil? && address_in(@msg["fromAddress"]) == ours
    end

    # Bare, down-cased address from a value that may be "Name <a@b>" or "a@b";
    # nil when there's no address to extract.
    def address_in(value)
      raw = value.to_s
      addr = (raw[/<([^>]+)>/, 1] || raw[/[^\s<>]+@[^\s<>]+/] || "").downcase.strip
      addr.empty? ? nil : addr
    end
  end
end
