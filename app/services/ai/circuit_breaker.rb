# frozen_string_literal: true

module Ai
  # Cache-backed circuit breaker for managed AI provider calls.
  #
  # == Semantics
  #
  #   CLOSED (normal): all callers proceed.
  #   OPEN: opened when TRIP_THRESHOLD 429 responses arrive within WINDOW_SECONDS.
  #     While open (for OPEN_TTL_SECONDS), background / bulk callers raise
  #     BackgroundBlocked immediately — fail fast, no provider hit; interactive
  #     callers (compose / Scout chat) always proceed.
  #
  # == Interactive context
  #
  # Wrap interactive AI work with Ai::CircuitBreaker.as_interactive { ... }.
  # The interactive chat reply jobs (AgentChatReplyJob, ComposeChatReplyJob,
  # EmailChatReplyJob, AiSetupChatReplyJob) do this automatically. Any caller
  # that does NOT wrap with as_interactive is treated as background and is
  # blocked when the breaker is open, causing the job to fail fast and
  # re-queue via its existing retry_on handler rather than hammering the
  # already-rate-limited provider.
  #
  # == Budget hook seam
  #
  # open?(provider:) exposes the breaker state so a future per-workspace
  # token-budget or spend-guard can hook the same check point without touching
  # the HTTP layer.
  #
  # == Cache backend
  #
  # Uses Rails.cache (Solid Cache in production — cross-process and durable
  # across restarts). Degrades gracefully on cache errors: check! fails open
  # (lets the call through) rather than causing false positives.
  module CircuitBreaker
    # Number of 429 responses in the tracking window that trips the breaker.
    TRIP_THRESHOLD = 5

    # Duration of the sliding tracking window. The counter TTL is set to this
    # value so it auto-resets when 429s stop arriving.
    WINDOW_SECONDS = 60

    # How long the breaker stays open (blocking background callers) before
    # auto-resetting. Should be longer than WINDOW_SECONDS so a burst does not
    # immediately re-trip the just-closed breaker.
    OPEN_TTL_SECONDS = 120

    # Thread-local key — set by as_interactive to mark the current execution
    # context as interactive (compose / Scout chat). Background jobs leave it
    # unset; the check defaults to background in that case, which is safer.
    INTERACTIVE_FLAG = :ai_circuit_interactive

    # Raised by check! when the breaker is open and the caller is background.
    # Handled by the job layer: jobs declare retry_on this error with backoff
    # so they re-queue without hammering the provider.
    #
    # IMPORTANT: inherits from Exception, NOT StandardError.
    # Background AI services (Ai::ContactAnalyzer, Ai::EmailClassifier,
    # Ai::ReminderExtractor, EmbeddingService, etc.) wrap their adapter call in
    # `rescue => e … nil`, which only catches StandardError descendants. Inheriting
    # from Exception makes BackgroundBlocked propagate PAST those service-level
    # rescue clauses, all the way up to the job's retry_on handler in ApplicationJob,
    # which re-queues the job cleanly instead of silently swallowing the blocked call.
    # Explicit `rescue Ai::CircuitBreaker::BackgroundBlocked` in
    # CircuitBreakerMiddleware (and anywhere else) still catches it — explicit-class
    # rescue works for any Exception subclass, not just StandardError.
    class BackgroundBlocked < Exception  # rubocop:disable Lint/InheritException
      def initialize(provider)
        super("AI circuit breaker OPEN for #{provider} — background call blocked; job will re-queue with backoff")
      end
    end

    # ── Public interface ───────────────────────────────────────────────────────

    # Wraps a block as interactive: the circuit breaker check is bypassed inside,
    # so compose / Scout replies are never blocked by background 429 storms.
    #
    # Usage (in chat reply jobs):
    #   Ai::CircuitBreaker.as_interactive { do_the_ai_call(...) }
    def self.as_interactive
      was = Thread.current[INTERACTIVE_FLAG]
      Thread.current[INTERACTIVE_FLAG] = true
      yield
    ensure
      Thread.current[INTERACTIVE_FLAG] = was
    end

    # Returns true when the current thread is in an interactive context.
    def self.interactive?
      Thread.current[INTERACTIVE_FLAG] == true
    end

    # Returns true when the breaker is currently open for this provider.
    # Exposed as a budget-check seam — call this without check! when you want
    # to inspect state rather than enforce the gate.
    def self.open?(provider:)
      Rails.cache.read(open_cache_key(provider)) == true
    rescue StandardError
      false # fail open on cache errors — better than blocking valid traffic
    end

    # Records a 429 response for a provider. Trips the breaker (writes the open
    # flag with OPEN_TTL_SECONDS) when TRIP_THRESHOLD is reached within the window.
    # Called by Ai::CircuitBreakerMiddleware after each 429 response or error.
    def self.track_429!(provider:)
      key = counter_cache_key(provider)

      # Read-increment-write: approximate under concurrent workers, acceptable
      # for a circuit breaker (slightly delayed trip vs. slightly early trip).
      count = Rails.cache.read(key).to_i + 1
      Rails.cache.write(key, count, expires_in: WINDOW_SECONDS.seconds)

      if count >= TRIP_THRESHOLD
        Rails.cache.write(open_cache_key(provider), true, expires_in: OPEN_TTL_SECONDS.seconds)
        Rails.logger.warn(
          "[Ai::CircuitBreaker] #{provider} — breaker OPENED " \
          "(#{count} 429s in #{WINDOW_SECONDS}s; will auto-reset in #{OPEN_TTL_SECONDS}s)"
        )
      end
    rescue StandardError => e
      Rails.logger.warn("[Ai::CircuitBreaker] track_429! failed (non-critical): #{e.message}")
    end

    # Fails fast for background callers when the breaker is open.
    # Interactive callers (as_interactive block) always pass through.
    # Called at the start of every AI HTTP request via CircuitBreakerMiddleware.
    def self.check!(provider:)
      return if interactive?
      return unless open?(provider: provider)

      raise BackgroundBlocked.new(provider)
    end

    # ── Cache key helpers (private) ────────────────────────────────────────────

    def self.counter_cache_key(provider) = "ai:cb:count:#{provider}"
    def self.open_cache_key(provider)    = "ai:cb:open:#{provider}"
    private_class_method :counter_cache_key, :open_cache_key
  end
end
