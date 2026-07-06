# frozen_string_literal: true

module Emails
  # Recognises Campbooks' OWN outbound mail — digests, notifications, transactional
  # — when it lands back in a connected mailbox, so the ingest pipeline can skip the
  # AI extractors (and flag digests for the reader) instead of mining our own mail
  # for the reminders / tasks / contacts it already lists.
  #
  # Detection is gated on the sender: the message must come FROM our own configured
  # mailer address (MAILER_FROM). That gate is SPF/DMARC-protected, so a third party
  # can't trip it by spoofing our headers. The X-Campbooks-Kind header — which we
  # stamp on the way out (DigestMailer) and which Gmail / Microsoft surface on the
  # way back in — only REFINES the kind (→ "digest") for mail already confirmed
  # ours; Zoho strips transport headers from its list endpoint, so its digests fall
  # back to the generic "campbooks" kind (still skipped, just not badged "Digest").
  #
  # Dependency-free (the sender address is injected) so it unit-tests in isolation,
  # like Emails::Categorizer.
  class SelfGeneratedDetector
    GENERIC_KIND = "campbooks"

    # Kinds a message is allowed to declare via X-Campbooks-Kind. Anything else
    # (including a spoofed value) collapses to the generic kind.
    KNOWN_KINDS = %w[digest].freeze

    def self.kind_for(msg, mailer_from:)
      new(msg, mailer_from: mailer_from).kind
    end

    def initialize(msg, mailer_from:)
      @msg = msg || {}
      @mailer_from = mailer_from
    end

    # The self-generated kind ("digest" or "campbooks"), or nil for third-party mail.
    def kind
      return nil unless from_our_mailer?

      declared = header("header_campbooks_kind")
      KNOWN_KINDS.include?(declared) ? declared : GENERIC_KIND
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
