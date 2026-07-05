# frozen_string_literal: true

# Faraday middleware that records every request as a SystemHealth log row,
# including sanitized request/response headers and bodies.
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
#   workspace:         (optional) workspace id, or a callable returning one.
#                      Account-bound clients pass the account's workspace so
#                      rows attribute correctly even outside job/request
#                      context (console, rake). Falls back to Current.workspace
#                      when nil.
#
# Body capture mechanics (Faraday 2.x):
#   The middleware is OUTERMOST. At call(env) time env[:body] is the raw
#   pre-serialization object (e.g. a Hash for f.request :json connections).
#   After @app.call(env) the inner JSON/url_encoded request middleware has
#   encoded the body and written it to env[:request_body] — so at on_complete
#   time response_env[:request_body] is the wire-format string. We capture
#   from there, falling back to serialising pre_body only when it is nil.
class SystemHealth::FaradayMiddleware < Faraday::Middleware
  def initialize(app, service:, expected_statuses: [], workspace: nil)
    super(app)
    @service           = service
    @expected_statuses = Array(expected_statuses)
    @workspace         = workspace
  end

  def call(env)
    start       = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    operation   = derive_operation(env)

    # Snapshot these BEFORE @app.call so we have them even if the call raises.
    pre_body    = env[:body]   # may be a Hash (pre-serialization) or String
    req_headers = SystemHealth.sanitize_headers(env.request_headers.to_h)
    req_ct      = env.request_headers["Content-Type"].to_s

    @app.call(env).on_complete do |response_env|
      http_status = response_env.status
      success     = (200..399).cover?(http_status) || @expected_statuses.include?(http_status)

      resp_ct = response_env.response_headers&.[]("content-type").to_s

      SystemHealth.record(
        service:          @service,
        status:           success ? :success : :error,
        operation:        operation,
        duration_ms:      elapsed_ms(start),
        http_status:      http_status,
        workspace_id:     resolved_workspace_id,
        request_headers:  req_headers,
        response_headers: SystemHealth.sanitize_headers(response_env.response_headers.to_h),
        request_body:     SystemHealth.sanitize_body(
                            wire_request_body(response_env, pre_body),
                            content_type: req_ct
                          ),
        response_body:    SystemHealth.sanitize_body(
                            response_env[:body],
                            content_type: resp_ct
                          ),
        metadata:         build_metadata(response_env)
      )
    end
  rescue StandardError => e
    duration_ms = elapsed_ms(start)
    http_status = extract_status(e)

    resp_headers, resp_body = extract_error_response(e)

    # Wire request body is available in env[:request_body] even after a raise
    # because the inner request middlewares (JSON encoder) ran before raise_error.
    req_body_str = wire_request_body(env, pre_body)

    if http_status && @expected_statuses.include?(http_status)
      SystemHealth.record(
        service:          @service,
        status:           :success,
        operation:        operation,
        duration_ms:      duration_ms,
        http_status:      http_status,
        workspace_id:     resolved_workspace_id,
        request_headers:  req_headers,
        response_headers: resp_headers,
        request_body:     SystemHealth.sanitize_body(req_body_str, content_type: req_ct),
        response_body:    resp_body
      )
    else
      SystemHealth.record(
        service:          @service,
        status:           :error,
        operation:        operation,
        duration_ms:      duration_ms,
        http_status:      http_status,
        error_class:      e.class.name,
        error_message:    SystemHealth.sanitize_message(e.message),
        workspace_id:     resolved_workspace_id,
        request_headers:  req_headers,
        response_headers: resp_headers,
        request_body:     SystemHealth.sanitize_body(req_body_str, content_type: req_ct),
        response_body:    resp_body
      )
    end

    raise
  end

  private

  # The workspace option wins over ambient context; SystemHealth.record falls
  # back to Current.workspace when this returns nil. Never raises — attribution
  # must not break the request.
  def resolved_workspace_id
    @workspace.respond_to?(:call) ? @workspace.call : @workspace
  rescue StandardError
    nil
  end

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

  # Returns the wire-format request body string from the env after inner
  # middlewares have run. env[:request_body] holds the serialized string once
  # f.request :json or f.request :url_encoded has processed it. Falls back to
  # serialising pre_body (the value seen before @app.call) if still nil.
  def wire_request_body(env, pre_body)
    wire = env[:request_body]
    return wire if wire.is_a?(String) && !wire.empty?

    # Fallback: pre_body is whatever was in env[:body] before @app.call.
    return pre_body if pre_body.is_a?(String)
    return nil if pre_body.nil?

    # Pre_body is a Hash/Array (pre-serialization); serialise it.
    JSON.generate(pre_body)
  rescue StandardError
    nil
  end

  # Builds the metadata hash: model (from request context) + token usage for ai_* services.
  def build_metadata(response_env)
    model = response_env.request&.context&.dig(:model)
    meta = model ? { model: model.to_s } : {}

    if @service.start_with?("ai_")
      merge_token_usage!(meta, response_env[:body])
    end

    meta.presence
  end

  # Tries to extract token usage fields from a parsed or unparsed response body.
  # Covers OpenAI-compatible (prompt_tokens/completion_tokens) and Anthropic
  # (input_tokens/output_tokens) shapes. Modifies meta in place.
  def merge_token_usage!(meta, body)
    parsed = body.is_a?(Hash) ? body : JSON.parse(body.to_s)
    usage  = parsed["usage"]
    return unless usage.is_a?(Hash)

    tokens_in  = usage["prompt_tokens"]  || usage["input_tokens"]
    tokens_out = usage["completion_tokens"] || usage["output_tokens"]
    meta[:tokens_in]  = tokens_in.to_i  if tokens_in
    meta[:tokens_out] = tokens_out.to_i if tokens_out
  rescue JSON::ParserError, TypeError
    # Body is not JSON or has no usage field — skip token extraction.
  end

  # Returns [sanitized_response_headers, sanitized_response_body] from a
  # Faraday exception. Both are nil for network errors that carry no response
  # (e.g. timeouts, connection failures — response is nil on the exception).
  def extract_error_response(exception)
    return [ nil, nil ] unless exception.is_a?(Faraday::Error)

    headers = exception.response_headers
    body    = exception.response_body

    # Network-level failures have no HTTP response at all.
    return [ nil, nil ] if headers.nil? && body.nil?

    ct = headers.to_h["content-type"].to_s

    [
      SystemHealth.sanitize_headers(headers.to_h),
      SystemHealth.sanitize_body(body, content_type: ct)
    ]
  end
end
