# frozen_string_literal: true

# System Health: records every interaction with an external service as a log
# row so an admin dashboard can show per-service success rates and error logs.
#
# Disable recording entirely by setting DISABLE_SYSTEM_HEALTH=1.
module SystemHealth
  # Request headers whose presence alone could expose credentials; dropped entirely
  # from stored rows (no "[REDACTED]" placeholder — their presence is not useful).
  HEADER_DENYLIST = %w[
    authorization proxy-authorization cookie set-cookie
    x-api-key api-key x-auth-token x-goog-api-key
  ].freeze

  # Maximum characters stored per request/response body. Redaction happens first;
  # truncation appends a marker so the reader knows the body was cut.
  BODY_LIMIT = 10_000

  # Service registry: key => group.
  # Groups: :email, :calendar, :storage, :ai, :automation, :notifications, :auth.
  # Unknown services fall back to :other.
  SERVICES = {
    "google_mail"       => :email,
    "zoho_mail"         => :email,
    "microsoft_mail"    => :email,
    "smtp"              => :email,
    "google_calendar"   => :calendar,
    "zoho_calendar"     => :calendar,
    "google_drive"      => :storage,
    "zoho_drive"        => :storage,
    "notion"            => :storage,
    "ai_anthropic"      => :ai,
    "ai_openai"         => :ai,
    "ai_mistral"        => :ai,
    "ai_gemini"         => :ai,
    "ai_deepseek"       => :ai,
    "slack"             => :automation,
    "discord"           => :automation,
    "webhook"           => :automation,
    "connection"        => :automation,
    "github"            => :automation,
    "push_apns"         => :notifications,
    "push_fcm"          => :notifications,
    "google_oauth"      => :auth,
    "zoho_oauth"        => :auth,
    "microsoft_oauth"   => :auth,
    "google_drive_oauth" => :auth,
    "notion_oauth"      => :auth
  }.freeze

  def self.enabled? = ENV["DISABLE_SYSTEM_HEALTH"] != "1"

  def self.group_for(service) = SERVICES.fetch(service, :other)

  # Writes a single log row. Never raises — if anything goes wrong (DB down,
  # table not yet migrated, validation error) it logs a warning and returns nil.
  # Returns the created row or nil.
  #
  # The four capture keyword args (request_headers, response_headers,
  # request_body, response_body) are ALWAYS-ON when provided by the middleware;
  # they are nil by default so non-HTTP callers (SystemHealth.track) are unaffected.
  def self.record(service:, status:, operation: nil, duration_ms: nil, http_status: nil,
                  error_class: nil, error_message: nil, workspace_id: nil, metadata: nil,
                  request_headers: nil, response_headers: nil,
                  request_body: nil, response_body: nil)
    return nil unless enabled?

    resolved_workspace_id = workspace_id || begin
      Current.workspace&.id
    rescue StandardError
      nil
    end

    ExternalServiceCall.create!(
      service:          service,
      status:           status,
      operation:        operation,
      duration_ms:      duration_ms,
      http_status:      http_status,
      error_class:      error_class,
      error_message:    error_message && sanitize_message(error_message),
      workspace_id:     resolved_workspace_id,
      metadata:         metadata || {},
      request_headers:  request_headers,
      response_headers: response_headers,
      request_body:     request_body,
      response_body:    response_body
    )
  rescue StandardError => e
    Rails.logger.warn("[SystemHealth] failed to record #{service}: #{e.class}: #{e.message}")
    nil
  end

  # Wraps a block: times it, records success, or records error and RE-RAISES.
  # Returns the block's return value on success.
  def self.track(service:, operation: nil, workspace_id: nil, metadata: nil)
    start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    result = yield
    duration_ms = elapsed_ms(start)
    record(
      service:      service,
      status:       :success,
      operation:    operation,
      duration_ms:  duration_ms,
      workspace_id: workspace_id,
      metadata:     metadata
    )
    result
  rescue StandardError => e
    duration_ms = elapsed_ms(start)
    record(
      service:       service,
      status:        :error,
      operation:     operation,
      duration_ms:   duration_ms,
      workspace_id:  workspace_id,
      metadata:      metadata,
      error_class:   e.class.name,
      error_message: sanitize_message(e.message)
    )
    raise
  end

  # Sanitizes a string for storage in error_message:
  #   1. Strips query strings from URLs (they routinely carry keys/tokens).
  #   2. Redacts key=value credential pairs, "Bearer <token>" and sk-… keys.
  #   3. Collapses internal whitespace.
  #   4. Truncates to ExternalServiceCall::MESSAGE_LIMIT characters.
  #
  # Deliberately scoped so ordinary prose survives: "Token has expired" or a
  # mid-sentence "?" must come through intact.
  def self.sanitize_message(message)
    return "" if message.nil?

    msg = message.to_s
    msg = msg.gsub(%r{(https?://[^\s?]+)\?\S+}i, '\1?[FILTERED]')
    msg = msg.gsub(/\b(?:client_secret|api[_-]?key|token|key|secret|password)\b\s*[=:]\s*\S+/i, "[FILTERED]")
    msg = msg.gsub(%r{\bBearer\s+[A-Za-z0-9._~+/=\-]{8,}}i, "Bearer [FILTERED]")
    msg = msg.gsub(/\bsk-[A-Za-z0-9_\-]{8,}/, "[FILTERED]")
    msg = msg.gsub(/\s+/, " ").strip
    msg.slice(0, ExternalServiceCall::MESSAGE_LIMIT)
  end

  # Returns a sanitized copy of an HTTP header hash with denylisted headers removed.
  # Keys are compared case-insensitively. Values are cast to String. Input may be
  # a Faraday::Utils::Headers or any hash-like object. Returns {} for nil/blank input.
  def self.sanitize_headers(hash)
    return {} if hash.blank?

    hash.each_with_object({}) do |(k, v), out|
      next if HEADER_DENYLIST.include?(k.to_s.downcase)

      out[k.to_s] = v.to_s
    end
  rescue StandardError
    {}
  end

  # Returns a sanitized, size-capped copy of an HTTP body string for storage.
  #
  # Processing order (all security decisions happen before truncation):
  #   1. Non-string values (Hash/Array from a JSON response middleware) are
  #      round-tripped through JSON.generate so the stored value is always text.
  #   2. Invalid UTF-8 or a non-text content-type returns a binary placeholder.
  #   3. Credential redaction:
  #        - URL query strings stripped (same rule as sanitize_message)
  #        - key=value credential pairs, Bearer tokens, sk-* keys redacted
  #        - JSON credential fields: "api_key":"..." -> "api_key":"[FILTERED]"
  #   4. Truncation to BODY_LIMIT with a size marker.
  #
  # Returns nil when raw is nil.
  def self.sanitize_body(raw, content_type: nil)
    return nil if raw.nil?

    # Normalize compound objects produced by response JSON middlewares.
    raw = if raw.is_a?(Hash) || raw.is_a?(Array)
      JSON.generate(raw)
    else
      raw.to_s
    end

    total_bytes = raw.bytesize

    # Ensure valid UTF-8; if not, treat as binary regardless of content-type.
    unless raw.encoding == Encoding::UTF_8 && raw.valid_encoding?
      coerced = raw.dup.force_encoding(Encoding::UTF_8)
      unless coerced.valid_encoding?
        ct = content_type.to_s.split(";").first&.strip.presence || "unknown"
        return "[binary #{ct}, #{total_bytes} bytes]"
      end
      raw = coerced
    end

    # Non-text content-type returns a placeholder (no body content in the log).
    if content_type.present? && !content_type.to_s.match?(/json|text|xml|x-www-form-urlencoded/i)
      ct = content_type.to_s.split(";").first&.strip || content_type.to_s
      return "[binary #{ct}, #{total_bytes} bytes]"
    end

    # Credential redaction (same patterns as sanitize_message, without whitespace collapse).
    body = raw.dup
    body = body.gsub(%r{(https?://[^\s?]+)\?\S+}i, '\1?[FILTERED]')
    body = body.gsub(%r{\bBearer\s+[A-Za-z0-9._~+/=\-]{8,}}i, "Bearer [FILTERED]")
    body = body.gsub(/\bsk-[A-Za-z0-9_\-]{8,}/, "[FILTERED]")

    if body.lstrip.start_with?("{", "[")
      # JSON bodies: redact by quoted field name. The bare key=value rule below
      # would eat everything to the next whitespace in minified JSON the moment
      # ordinary content mentions "password:" — gutting exactly the bodies this
      # log exists to show — so it applies to non-JSON bodies only.
      body = body.gsub(/"(api_?key|access_token|refresh_token|client_secret|id_token|password|secret|token)"\s*:\s*"[^"]*"/i) do
        "\"#{$1}\":\"[FILTERED]\""
      end
    else
      body = body.gsub(/\b(?:client_secret|api[_-]?key|token|key|secret|password)\b\s*[=:]\s*\S+/i, "[FILTERED]")
    end

    # Truncate after redaction (so a secret never straddles the cut point).
    if body.length > BODY_LIMIT
      body = "#{body[0, BODY_LIMIT]} ...[truncated, #{total_bytes} bytes total]"
    end

    body
  rescue StandardError => e
    "[body sanitization failed: #{e.class}]"
  end

  def self.elapsed_ms(start)
    ((Process.clock_gettime(Process::CLOCK_MONOTONIC) - start) * 1000).round
  end
  private_class_method :elapsed_ms
end
