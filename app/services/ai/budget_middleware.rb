# frozen_string_literal: true

module Ai
  # Faraday middleware that enforces per-workspace daily managed-AI call budgets.
  #
  # Sits as a sibling to CircuitBreakerMiddleware in Ai::Adapters::Base#connection,
  # declared AFTER the circuit breaker so the breaker's fast-path runs first:
  #
  #   f.use Ai::CircuitBreakerMiddleware, provider: provider_name
  #   f.use Ai::BudgetMiddleware
  #
  # == What it enforces
  #
  # Before the call: Ai::Budget.enforce!(workspace:) raises Exceeded when the
  # workspace's daily managed-AI call counter has reached its cap.
  #
  # After a successful call: Ai::Budget.record!(workspace:) increments the counter.
  # The counter is only incremented on success — a failed call (provider error,
  # network timeout, etc.) is not counted, consistent with not charging for
  # incomplete work.
  #
  # == Managed-only enforcement
  #
  # Enforcement is skipped entirely when:
  #   - Current.workspace is nil (background jobs without workspace context)
  #   - self_hosted? (operators pay their own keys; no vendor spend)
  #   - the workspace uses a BYO key (Ai::Budget.cap_for returns Float::INFINITY)
  #
  # == Fail-open on cache errors
  #
  # Ai::Budget.enforce! and Ai::Budget.record! both rescue cache errors internally
  # and fail open, so a cache hiccup never blocks real AI traffic.
  #
  # == Error propagation
  #
  # Exceeded inherits from Exception (not StandardError), so it bypasses the
  # pervasive `rescue => e … nil` clauses in background AI services and reaches
  # the job layer (ApplicationJob#discard_on). Interactive chat reply jobs
  # rescue it explicitly and post a friendly message to the user.
  class BudgetMiddleware < Faraday::Middleware
    def call(env)
      workspace = Current.workspace
      Ai::Budget.enforce!(workspace: workspace)

      @app.call(env).tap do
        Ai::Budget.record!(workspace: workspace)
      end
    rescue Ai::Budget::Exceeded
      raise # propagate to job discard_on handler or interactive rescue
    end
  end
end
