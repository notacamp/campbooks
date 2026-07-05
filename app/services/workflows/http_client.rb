module Workflows
  # Thin, safe Faraday wrapper used by every outbound workflow action. It never
  # raises on HTTP-level failures — it always returns a normalized result hash
  # so the executor can record what happened:
  #
  #   { ok:, status:, headers:, body:, error: }
  #
  # `ok` is true only for a 2xx response. Transport problems (timeouts, DNS,
  # blocked URLs) come back with status 0 and a populated `error`.
  class HttpClient
    DEFAULT_TIMEOUT = 10
    MAX_OPEN_TIMEOUT = 5
    MAX_BODY_BYTES = 64 * 1024

    def self.call(method:, url:, headers: {}, body: nil, timeout: DEFAULT_TIMEOUT, connection: nil, service: "webhook")
      new(connection, service: service).call(method: method, url: url, headers: headers, body: body, timeout: timeout)
    end

    def initialize(connection = nil, service: "webhook")
      @connection = connection
      @service = service
    end

    def call(method:, url:, headers: {}, body: nil, timeout: DEFAULT_TIMEOUT)
      uri = UrlGuard.validate!(url)
      verb = method.to_s.downcase.to_sym

      response = connection(timeout).run_request(verb, uri.to_s, body, headers)

      {
        ok: response.success?,
        status: response.status,
        headers: response.headers.to_h,
        body: clean_body(response.body),
        error: nil
      }
    rescue UrlGuard::BlockedError => e
      blocked_host = begin
        URI.parse(url).host
      rescue StandardError
        "unknown"
      end
      SystemHealth.record(
        service: @service,
        operation: "BLOCKED #{method.to_s.upcase} #{blocked_host}",
        status: :error,
        error_class: e.class.name,
        error_message: SystemHealth.sanitize_message(e.message)
      )
      failure(e.message)
    rescue Faraday::TimeoutError
      failure("Request timed out after #{timeout}s")
    rescue Faraday::ConnectionFailed => e
      failure("Connection failed: #{e.message}")
    rescue Faraday::Error => e
      failure(e.message)
    end

    private

    def failure(message)
      { ok: false, status: 0, headers: {}, body: "", error: message }
    end

    def clean_body(body)
      body.to_s.scrub.truncate_bytes(MAX_BODY_BYTES)
    end

    def connection(timeout)
      @connection ||= Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: @service
        f.options.timeout = timeout
        f.options.open_timeout = [ timeout, MAX_OPEN_TIMEOUT ].min
        f.adapter Faraday.default_adapter
      end
    end
  end
end
