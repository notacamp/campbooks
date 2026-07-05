# frozen_string_literal: true

# System Health: records every interaction with an external service as a log
# row so an admin dashboard can show per-service success rates and error logs.
#
# Disable recording entirely by setting DISABLE_SYSTEM_HEALTH=1.
module SystemHealth
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
  def self.record(service:, status:, operation: nil, duration_ms: nil, http_status: nil,
                  error_class: nil, error_message: nil, workspace_id: nil, metadata: nil)
    return nil unless enabled?

    resolved_workspace_id = workspace_id || begin
      Current.workspace&.id
    rescue StandardError
      nil
    end

    ExternalServiceCall.create!(
      service:        service,
      status:         status,
      operation:      operation,
      duration_ms:    duration_ms,
      http_status:    http_status,
      error_class:    error_class,
      error_message:  error_message && sanitize_message(error_message),
      workspace_id:   resolved_workspace_id,
      metadata:       metadata || {}
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

  def self.elapsed_ms(start)
    ((Process.clock_gettime(Process::CLOCK_MONOTONIC) - start) * 1000).round
  end
  private_class_method :elapsed_ms
end
