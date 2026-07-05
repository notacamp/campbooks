# frozen_string_literal: true

# Faraday middleware that records every request as a SystemHealth log row.
#
# Usage (always place FIRST in the builder block so it is outermost and sees
# exceptions raised by f.response :raise_error declared after it):
#
#   Faraday.new do |f|
#     f.use SystemHealth::FaradayMiddleware, service: "google_mail"
#     f.response :raise_error
#     f.adapter Faraday.default_adapter
#   end
#
# Options:
#   service:           (required) String key from SystemHealth::SERVICES.
#   expected_statuses: (optional) Array of Integer HTTP statuses treated as
#                      success even when raise_error raises for them (e.g. 410
#                      for Google sync-token expiry). The exception is still
#                      re-raised; only the recorded status changes.
class SystemHealth::FaradayMiddleware < Faraday::Middleware
  def initialize(app, service:, expected_statuses: [])
    super(app)
    @service          = service
    @expected_statuses = Array(expected_statuses)
  end

  def call(env)
    start     = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    operation = derive_operation(env)

    @app.call(env).on_complete do |response_env|
      http_status = response_env.status
      success     = (200..399).cover?(http_status) || @expected_statuses.include?(http_status)

      SystemHealth.record(
        service:     @service,
        status:      success ? :success : :error,
        operation:   operation,
        duration_ms: elapsed_ms(start),
        http_status: http_status
      )
    end
  rescue StandardError => e
    duration_ms = elapsed_ms(start)
    http_status = extract_status(e)

    if http_status && @expected_statuses.include?(http_status)
      SystemHealth.record(
        service:     @service,
        status:      :success,
        operation:   operation,
        duration_ms: duration_ms,
        http_status: http_status
      )
    else
      SystemHealth.record(
        service:       @service,
        status:        :error,
        operation:     operation,
        duration_ms:   duration_ms,
        http_status:   http_status,
        error_class:   e.class.name,
        error_message: SystemHealth.sanitize_message(e.message)
      )
    end

    raise
  end

  private

  def elapsed_ms(start)
    ((Process.clock_gettime(Process::CLOCK_MONOTONIC) - start) * 1000).round
  end

  # Returns the HTTP status from a Faraday exception, or nil for network-level
  # failures (timeouts, connection errors) that carry no response.
  def extract_status(exception)
    return nil unless exception.is_a?(Faraday::Error) && exception.respond_to?(:response_status)

    exception.response_status
  end

  # Derives a normalized operation string ("METHOD /sanitized/path") from the
  # request environment. Returns nil rather than raising — a recording detail
  # must never break the request itself.
  def derive_operation(env)
    url = env&.url
    return nil unless url

    "#{env.method.to_s.upcase} #{sanitize_path(url.path)}".slice(0, 200)
  rescue StandardError
    nil
  end

  # Replaces path segments that look like IDs with :id so operation strings
  # do not blow up cardinality in the dashboard.
  #
  # A segment is an ID when it:
  #   - consists entirely of digits, or
  #   - is 16+ hex characters / UUID-ish (/\A[0-9a-f-]{16,}\z/i), or
  #   - contains an "@" character, or
  #   - is longer than 24 characters.
  def sanitize_path(path)
    return "" if path.nil?

    segments = path.split("/").map do |segment|
      next segment if segment.empty?
      next ":id" if id_segment?(segment)

      segment
    end

    segments.join("/")
  end

  def id_segment?(segment)
    return true if segment.match?(/\A\d+\z/)
    return true if segment.match?(/\A[0-9a-f\-]{16,}\z/i)
    return true if segment.include?("@")
    return true if segment.length > 24

    false
  end
end
