# frozen_string_literal: true

module Ai
  # Per-workspace daily spend-guard for the shared managed ("Campbooks AI") key.
  #
  # Prevents a single workspace from draining the vendor-paid AI budget.
  # Self-hosted installs and workspaces on BYO keys pay their own AI costs and
  # are never subject to these limits — they always resolve Float::INFINITY.
  #
  # == Two configurable caps
  #
  # 1. Global ceiling (AI_DAILY_CALL_CEILING, default 10_000):
  #    Applied to every managed-cloud workspace. High enough that normal use
  #    never hits it; low enough to bound worst-case abuse.
  #
  # 2. New-account probation (AI_PROBATION_DAILY_CAP, default UNSET = disabled):
  #    An optional tighter cap for workspaces created within the last
  #    AI_PROBATION_DAYS (default 7) days. Off by default — set
  #    AI_PROBATION_DAILY_CAP to an integer to enable.
  #
  # == Cache backend
  #
  # Rolling counters live in Rails.cache (Solid Cache in production — durable
  # across restarts, shared across workers). TTL is 48 hours so the key
  # auto-expires well after the day rolls over. All cache errors fail open
  # (never block real traffic due to a cache hiccup).
  #
  # == Exception hierarchy
  #
  # Exceeded inherits from Exception, NOT StandardError.
  # Background AI services (Ai::ContactAnalyzer, Ai::EmailClassifier, etc.)
  # wrap their adapter call in `rescue => e … nil`, which only catches
  # StandardError descendants. Inheriting from Exception lets Exceeded propagate
  # PAST those service-level rescue clauses, all the way up to the job layer
  # where `discard_on Ai::Budget::Exceeded` (ApplicationJob) discards it cleanly
  # and logs a warning. Interactive chat jobs rescue it explicitly and post a
  # friendly i18n reply to the user instead.
  #
  # == Managed vs BYO detection
  #
  # cap_for(workspace) returns INFINITY when:
  #   - Rails.application.config.self_hosted is true  (operator pays their own keys)
  #   - Ai::ProviderSetup.new(workspace).using_managed? is false  (BYO key workspace)
  #
  # using_managed? checks whether the workspace's text role adapter has
  # AiAdapter#managed? == true (a DB column set by apply_managed / the cloud
  # provisioning flow). A BYO workspace that entered its own key has managed: false
  # and is therefore excluded from budget enforcement entirely.
  module Budget
    # ── ENV-controlled configuration ────────────────────────────────────────────

    # Hard daily ceiling for all managed-cloud workspaces. Set AI_DAILY_CALL_CEILING
    # to override. Default is intentionally very high so normal use never trips it;
    # set lower in staging/sandbox environments to test enforcement paths.
    DEFAULT_DAILY_CEILING = 10_000

    # Probation period in days for new self-serve workspaces. Set AI_PROBATION_DAYS
    # to override. Only applies when AI_PROBATION_DAILY_CAP is also set.
    DEFAULT_PROBATION_DAYS = 7

    # ── Exceptions ──────────────────────────────────────────────────────────────

    # Raised by enforce! when a workspace has consumed its daily managed-AI cap.
    #
    # IMPORTANT: inherits from Exception, NOT StandardError.
    # Background AI services (Ai::ContactAnalyzer, Ai::EmailClassifier,
    # Ai::ReminderExtractor, EmbeddingService, etc.) wrap their adapter call in
    # `rescue => e … nil`, which only catches StandardError descendants. Inheriting
    # from Exception makes Exceeded propagate PAST those service-level rescue clauses,
    # all the way up to the job's discard_on handler in ApplicationJob, which discards
    # the job cleanly. Explicit `rescue Ai::Budget::Exceeded` (in BudgetMiddleware and
    # the interactive chat reply jobs) still catches it — explicit-class rescue works
    # for any Exception subclass.
    class Exceeded < Exception  # rubocop:disable Lint/InheritException
      def initialize(workspace_id)
        super("Ai::Budget: daily managed-AI cap reached for workspace #{workspace_id} — call discarded; cap resets tomorrow")
      end
    end

    # ── Public interface ─────────────────────────────────────────────────────────

    # Increment the per-workspace daily counter.
    # Called after every successful managed AI HTTP call (in BudgetMiddleware).
    # Cache errors are rescued and logged but never propagate — fail-open.
    def self.record!(workspace:)
      return unless workspace

      Rails.cache.increment(counter_key(workspace), 1, expires_in: 48.hours, raw: true)
    rescue StandardError => e
      Rails.logger.warn("[Ai::Budget] record! cache error (non-critical): #{e.message}")
    end

    # Returns the effective daily cap for this workspace, or Float::INFINITY when
    # enforcement does not apply (self-hosted, BYO-key, or nil workspace).
    def self.cap_for(workspace)
      return Float::INFINITY if workspace.nil?
      return Float::INFINITY if Rails.application.config.self_hosted
      return Float::INFINITY unless Ai::ProviderSetup.new(workspace).using_managed?

      ceiling = ENV.fetch("AI_DAILY_CALL_CEILING", DEFAULT_DAILY_CEILING).to_i

      probation_cap = probation_cap_for(workspace)
      return [ ceiling, probation_cap ].min if probation_cap

      ceiling
    end

    # Returns true when the workspace has consumed its daily cap.
    # Always returns false for self-hosted / BYO / nil workspace.
    def self.exceeded?(workspace)
      cap = cap_for(workspace)
      return false if cap == Float::INFINITY

      current_count(workspace) >= cap
    rescue StandardError => e
      Rails.logger.warn("[Ai::Budget] exceeded? cache error (fail-open): #{e.message}")
      false
    end

    # Raises Exceeded when the workspace has consumed its daily cap.
    # Called before each managed AI HTTP call (in BudgetMiddleware).
    def self.enforce!(workspace:)
      raise Exceeded.new(workspace.id) if exceeded?(workspace)
    end

    # ── Private helpers ──────────────────────────────────────────────────────────

    # Cache key for the rolling daily counter: includes the workspace id and the
    # current UTC date so the counter auto-resets overnight without a scheduled
    # task (the 48-hour TTL ensures old keys expire cleanly too).
    def self.counter_key(workspace)
      "ai:budget:#{workspace.id}:#{Time.now.utc.strftime('%Y%m%d')}"
    end
    private_class_method :counter_key

    def self.current_count(workspace)
      Rails.cache.read(counter_key(workspace)).to_i
    rescue StandardError
      0
    end
    private_class_method :current_count

    # Returns the probation cap integer if this workspace is in probation AND
    # AI_PROBATION_DAILY_CAP is configured; nil otherwise.
    # Probation: a managed-cloud workspace created within AI_PROBATION_DAYS days.
    def self.probation_cap_for(workspace)
      cap_env = ENV["AI_PROBATION_DAILY_CAP"].presence
      return nil unless cap_env

      days = ENV.fetch("AI_PROBATION_DAYS", DEFAULT_PROBATION_DAYS).to_i
      cutoff = days.days.ago
      return nil unless workspace.created_at >= cutoff

      cap_env.to_i
    end
    private_class_method :probation_cap_for
  end
end
