require "ipaddr"
require "uri"
require "resolv"
require "timeout"
require "socket"

module Workflows
  # Best-effort SSRF guard for outbound workflow HTTP. Workflows let users type
  # arbitrary URLs, so before we make a server-side request we reject the
  # obvious footguns: non-http(s) schemes, loopback/private/link-local/unspecified
  # IPs (in literal, bracketed-IPv6, or IPv4-mapped-IPv6 form), internal
  # hostnames (the cloud metadata endpoint, *.local, etc.), and public hostnames
  # whose DNS resolves to any of those.
  #
  # Local addresses are permitted in development so the app can be wired up
  # against a service running on the same machine; everywhere else they're
  # blocked.
  #
  # Residual limitations (accepted): this validates the URL we're about to call
  # but Faraday re-resolves DNS when it connects, so a rebinding attacker that
  # flips the record between our check and the connect could still slip through
  # (full mitigation requires pinning the connection to the validated IP). The
  # workflow HttpClient does NOT follow redirects, so a 30x to an internal host
  # is returned to the caller rather than fetched.
  class UrlGuard
    BlockedError = Class.new(StandardError)

    BLOCKED_HOSTS = %w[localhost metadata.google.internal metadata].freeze
    BLOCKED_SUFFIXES = %w[.local .internal .localhost].freeze
    METADATA_IP = IPAddr.new("169.254.169.254")
    # RFC 6598 carrier-grade NAT. Ruby's IPAddr#private? does NOT cover this range,
    # but it is routable to internal infra on cloud/Docker hosts, so block it.
    CGNAT = IPAddr.new("100.64.0.0/10")
    UNSPECIFIED_IPS = [ IPAddr.new("0.0.0.0"), IPAddr.new("::") ].freeze
    DNS_TIMEOUT = 2

    def self.validate!(url)
      new(url).validate!
    end

    def initialize(url)
      @raw = url.to_s.strip
    end

    def validate!
      raise BlockedError, "URL is required" if @raw.blank?

      uri = URI.parse(@raw)
      unless %w[http https].include?(uri.scheme)
        raise BlockedError, "Only http and https URLs are allowed"
      end
      raise BlockedError, "URL is missing a host" if uri.host.blank?
      raise BlockedError, "Refusing to call internal host: #{uri.host}" if blocked_host?(uri.host)

      uri
    rescue URI::InvalidURIError
      raise BlockedError, "Invalid URL: #{@raw}"
    end

    private

    def blocked_host?(host)
      return false if allow_local?

      # URI keeps IPv6 hosts bracketed ("[::1]"); strip them so IPAddr can parse.
      # A trailing dot is a valid FQDN terminator ("127.0.0.1." / "localhost." /
      # "metadata.google.internal.") that the system resolver still honours at
      # connect time — strip it before any check or it slips every comparison below.
      h = host.downcase.delete_prefix("[").delete_suffix("]").chomp(".")
      return true if BLOCKED_HOSTS.include?(h)
      return true if BLOCKED_SUFFIXES.any? { |s| h.end_with?(s) }

      # A literal IP is classified directly; a hostname is resolved and blocked
      # if ANY of its addresses are internal (defeats a public name with an A/AAAA
      # record pointing at loopback/private/metadata).
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
      # IPv4-mapped IPv6 (e.g. ::ffff:127.0.0.1, ::ffff:169.254.169.254) reports
      # false for loopback?/private?/link_local? — unwrap to the embedded IPv4
      # (low 32 bits) and classify that. Avoids IPAddr#native, which calls the
      # deprecated #ipv4_compat?.
      ip = IPAddr.new(ip.to_i & 0xffff_ffff, Socket::AF_INET) if ip.ipv6? && ip.ipv4_mapped?

      UNSPECIFIED_IPS.include?(ip) ||
        ip.loopback? || ip.private? || ip.link_local? || ip == METADATA_IP ||
        (ip.ipv4? && CGNAT.include?(ip))
    rescue IPAddr::InvalidAddressError
      false # not a literal IP — hostname checks already handled it
    end

    def resolved_addresses(host)
      Timeout.timeout(DNS_TIMEOUT) { Resolv.getaddresses(host) }
    rescue Timeout::Error, Resolv::ResolvError, SocketError
      [] # transient/failed resolution: let Faraday attempt and fail at connect
    end

    def allow_local?
      Rails.env.development?
    end
  end
end
