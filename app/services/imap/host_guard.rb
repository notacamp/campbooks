# frozen_string_literal: true

require "ipaddr"
require "resolv"
require "timeout"
require "socket"

module Imap
  # SSRF guard for IMAP/SMTP host strings. Mirrors the logic of
  # Workflows::UrlGuard but operates on a bare hostname/IP (no URI/scheme).
  #
  # Deliberate difference from UrlGuard: self-hosted installs are also allowed
  # unconditionally (in addition to development mode). A self-hosted operator
  # legitimately runs a mail server on a private/local host; cloud-hosted
  # Campbooks must block that path because it would be an SSRF vector against
  # the hosting infrastructure. UrlGuard only allows development because
  # workflow HTTP calls from a self-hosted instance are also external-facing.
  class HostGuard
    BlockedError = Class.new(StandardError)

    BLOCKED_HOSTS    = %w[localhost metadata.google.internal metadata].freeze
    BLOCKED_SUFFIXES = %w[.local .internal .localhost].freeze
    METADATA_IP      = IPAddr.new("169.254.169.254")
    # RFC 6598 carrier-grade NAT. IPAddr#private? does NOT cover 100.64.0.0/10,
    # but cloud and Docker hosts can route it to internal services, so we block it.
    CGNAT            = IPAddr.new("100.64.0.0/10")
    UNSPECIFIED_IPS  = [ IPAddr.new("0.0.0.0"), IPAddr.new("::") ].freeze
    DNS_TIMEOUT      = 2

    def self.validate!(host)
      new(host).validate!
    end

    def initialize(host)
      @raw = host.to_s.strip
    end

    def validate!
      raise BlockedError, "Host is required" if @raw.blank?
      raise BlockedError, "Refusing to connect to internal host: #{@raw}" if blocked_host?(@raw)
    end

    private

    def blocked_host?(host)
      return false if allow_local?

      # Strip a trailing FQDN dot ("mail.example.com.") and bracketed IPv6
      # ("[::1]") so they cannot slip past the literal-match checks below.
      h = host.downcase.delete_prefix("[").delete_suffix("]").chomp(".")
      return true if BLOCKED_HOSTS.include?(h)
      return true if BLOCKED_SUFFIXES.any? { |s| h.end_with?(s) }

      # Literal IPs are classified directly; hostnames are resolved and blocked
      # if ANY resolved address is internal (defeats a public hostname pointing
      # at loopback/private/metadata via DNS).
      if literal_ip?(h)
        internal_ip?(h)
      else
        resolved_addresses(h).any? { |ip| internal_ip?(ip) }
      end
    end

    def literal_ip?(host)
      IPAddr.new(host)
      true
    rescue IPAddr::InvalidAddressError
      false
    end

    def internal_ip?(host)
      ip = IPAddr.new(host.to_s)
      # IPv4-mapped IPv6 (e.g. ::ffff:127.0.0.1) reports false for loopback?/
      # private?/link_local? — unwrap to the embedded IPv4 (low 32 bits) so the
      # classification is correct.
      ip = IPAddr.new(ip.to_i & 0xffff_ffff, Socket::AF_INET) if ip.ipv6? && ip.ipv4_mapped?

      UNSPECIFIED_IPS.include?(ip) ||
        ip.loopback? || ip.private? || ip.link_local? || ip == METADATA_IP ||
        (ip.ipv4? && CGNAT.include?(ip))
    rescue IPAddr::InvalidAddressError
      false
    end

    def resolved_addresses(host)
      Timeout.timeout(DNS_TIMEOUT) { Resolv.getaddresses(host) }
    rescue Timeout::Error, Resolv::ResolvError, SocketError
      # DNS failure: let the TCP connect attempt fail with a clearer error; do not block.
      []
    end

    def allow_local?
      Rails.env.development? || Rails.application.config.self_hosted
    end
  end
end
