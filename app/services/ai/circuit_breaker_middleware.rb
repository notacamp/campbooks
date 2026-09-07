# frozen_string_literal: true

module Ai
  # Faraday middleware that integrates the circuit breaker into every AI adapter
  # HTTP connection. Sits outermost (declared first in the builder block, before
  # SystemHealth::FaradayMiddleware) so that:
  #
  #   1. Background callers that hit an open breaker raise BackgroundBlocked
  #      immediately, before any SystemHealth log row is written or any network
  #      socket is opened — true fail-fast.
  #
  #   2. Every 429 response/error is recorded via Ai::CircuitBreaker.track_429!
  #      so the breaker trips automatically after TRIP_THRESHOLD hits.
  #
  # Note: SystemHealth::FaradayMiddleware still records the 429 as an error row
  # (it also rescues Faraday::TooManyRequestsError); both middlewares co-exist
  # because the circuit breaker re-raises after recording.
  class CircuitBreakerMiddleware < Faraday::Middleware
    def initialize(app, provider:)
      super(app)
      @provider = provider
    end

    def call(env)
      # Fast-path: fail immediately for background callers when breaker is open.
      Ai::CircuitBreaker.check!(provider: @provider)

      @app.call(env)
    rescue Ai::CircuitBreaker::BackgroundBlocked
      raise # propagate to job retry_on handler
    rescue Faraday::TooManyRequestsError
      # The raise_error middleware already raised — record the 429 and re-raise.
      Ai::CircuitBreaker.track_429!(provider: @provider)
      raise
    end
  end
end
