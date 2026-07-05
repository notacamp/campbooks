# frozen_string_literal: true

# Lookbook previews for the System Health components. All structs are built
# directly from the Snapshot struct constants — no DB rows required.
class SystemHealthComponentPreview < ViewComponent::Preview
  # ── ServiceCard variants ──────────────────────────────────────────────────

  def service_card_healthy
    render Campbooks::SystemHealth::ServiceCard.new(entry: healthy_entry, log_path: "/admin/system_health")
  end

  def service_card_degraded
    render Campbooks::SystemHealth::ServiceCard.new(entry: degraded_entry, log_path: "/admin/system_health")
  end

  def service_card_failing
    render Campbooks::SystemHealth::ServiceCard.new(entry: failing_entry, log_path: "/admin/system_health")
  end

  def service_card_with_last_error
    render Campbooks::SystemHealth::ServiceCard.new(entry: entry_with_error, log_path: "/admin/system_health")
  end

  # ── CallRow variants ──────────────────────────────────────────────────────

  def call_row_success
    render Campbooks::SystemHealth::CallRow.new(call: success_call)
  end

  def call_row_error
    render Campbooks::SystemHealth::CallRow.new(call: error_call)
  end

  private

  def buckets(errors_at: [])
    now = Time.current.utc.beginning_of_hour
    24.times.map do |i|
      hour    = now - (23 - i) * 3600
      total   = rand(10..60)
      errors  = errors_at.include?(i) ? rand(2..8) : 0
      ::SystemHealth::Snapshot::Bucket.new(starts_at: hour, total: total, errors: errors)
    end
  end

  def healthy_entry
    ::SystemHealth::Snapshot::ServiceStat.new(
      service:         "google_mail",
      group:           :email,
      state:           :healthy,
      total:           1204,
      errors:          2,
      error_rate:      0.0017,
      avg_duration_ms: 320,
      last_error:      nil,
      last_success_at: 5.minutes.ago,
      buckets:         buckets
    )
  end

  def degraded_entry
    ::SystemHealth::Snapshot::ServiceStat.new(
      service:         "ai_openai",
      group:           :ai,
      state:           :degraded,
      total:           480,
      errors:          35,
      error_rate:      0.073,
      avg_duration_ms: 1140,
      last_error:      nil,
      last_success_at: 10.minutes.ago,
      buckets:         buckets(errors_at: [ 18, 19, 20 ])
    )
  end

  def failing_entry
    ::SystemHealth::Snapshot::ServiceStat.new(
      service:         "notion",
      group:           :storage,
      state:           :failing,
      total:           12,
      errors:          12,
      error_rate:      1.0,
      avg_duration_ms: nil,
      last_error:      nil,
      last_success_at: nil,
      buckets:         buckets(errors_at: [ 20, 21, 22, 23 ])
    )
  end

  def entry_with_error
    last_err = ::SystemHealth::Snapshot::LastError.new(
      at:            45.minutes.ago,
      error_class:   "Faraday::ServerError",
      error_message: "the server responded with status 503",
      http_status:   503,
      operation:     "POST /gmail/v1/users/me/messages/send"
    )

    ::SystemHealth::Snapshot::ServiceStat.new(
      service:         "google_mail",
      group:           :email,
      state:           :degraded,
      total:           800,
      errors:          48,
      error_rate:      0.06,
      avg_duration_ms: 410,
      last_error:      last_err,
      last_success_at: 2.minutes.ago,
      buckets:         buckets(errors_at: [ 19, 20, 21 ])
    )
  end

  def success_call
    ExternalServiceCall.new(
      id:          SecureRandom.uuid,
      service:     "google_mail",
      status:      :success,
      operation:   "GET /gmail/v1/users/me/messages",
      duration_ms: 320,
      http_status: 200,
      created_at:  3.minutes.ago
    )
  end

  def error_call
    ExternalServiceCall.new(
      id:            SecureRandom.uuid,
      service:       "ai_openai",
      status:        :error,
      operation:     "POST /v1/chat/completions",
      duration_ms:   1800,
      http_status:   429,
      error_class:   "Faraday::TooManyRequestsError",
      error_message: "rate limit exceeded, retry after 60s",
      created_at:    7.minutes.ago
    )
  end
end
