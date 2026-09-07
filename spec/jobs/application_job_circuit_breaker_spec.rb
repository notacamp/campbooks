# frozen_string_literal: true

require "rails_helper"

# Spec: verifies that the circuit breaker wiring in ApplicationJob is correct —
# specifically that BackgroundBlocked propagates past service-level `rescue => e`
# clauses (because it inherits from Exception, not StandardError) and that the
# ApplicationJob retry_on handler re-queues it cleanly.
RSpec.describe "ApplicationJob circuit breaker wiring" do
  # ── BackgroundBlocked exception hierarchy ─────────────────────────────────────

  describe "Ai::CircuitBreaker::BackgroundBlocked" do
    subject(:error) { Ai::CircuitBreaker::BackgroundBlocked.new("mistral") }

    it "is an Exception (not a StandardError), so it propagates past rescue => e" do
      expect(error).to be_a(Exception)
    end

    it "is NOT a StandardError descendant (the key safety property)" do
      expect(error).not_to be_a(StandardError)
    end

    it "includes the provider name in the message" do
      expect(error.message).to include("mistral")
    end

    it "is caught by explicit rescue Ai::CircuitBreaker::BackgroundBlocked" do
      raised = false
      begin
        raise Ai::CircuitBreaker::BackgroundBlocked.new("mistral")
      rescue Ai::CircuitBreaker::BackgroundBlocked
        raised = true
      end
      expect(raised).to be(true)
    end

    it "is NOT caught by a bare rescue => e (StandardError rescue)" do
      caught_by_standard = false
      begin
        raise Ai::CircuitBreaker::BackgroundBlocked.new("mistral")
      rescue StandardError
        caught_by_standard = true
      rescue Exception # rubocop:disable Lint/SuppressedException
        # Expected: propagates past StandardError
      end
      expect(caught_by_standard).to be(false)
    end

    it "propagates through a typical service-level rescue-all guard" do
      # Simulates what the background AI services do:
      #   begin; adapter_call; rescue => e; nil; end
      # With BackgroundBlocked inheriting from Exception, the nil is never returned.
      def service_call_with_swallow_guard
        raise Ai::CircuitBreaker::BackgroundBlocked.new("mistral")
      rescue => e # rubocop:disable Lint/SuppressedException, Style/RescueStandardError
        # This rescue MUST NOT catch BackgroundBlocked.
        nil
      end

      expect { service_call_with_swallow_guard }.to raise_error(Ai::CircuitBreaker::BackgroundBlocked)
    end
  end

  # ── ApplicationJob retry_on registration ─────────────────────────────────────

  describe ApplicationJob do
    it "declares retry_on for Ai::CircuitBreaker::BackgroundBlocked" do
      # ActiveJob stores retry handlers in a class-level array. We verify the
      # correct exception class is registered so we don't need to fire a real job.
      retry_handlers = described_class.rescue_handlers.map(&:first)
      expect(retry_handlers).to include("Ai::CircuitBreaker::BackgroundBlocked")
    end
  end

  # ── End-to-end: a concrete background job re-queues on breaker-open ──────────

  describe "background job re-queue behaviour" do
    # A minimal job that makes a background AI call (not wrapped in as_interactive).
    class BreakerTestBackgroundJob < ApplicationJob
      queue_as :default

      def perform
        # Simulates a background AI service call that hits the open breaker.
        Ai::CircuitBreaker.check!(provider: "mistral")
      end
    end

    around do |example|
      original = Rails.cache
      Rails.cache = ActiveSupport::Cache::MemoryStore.new
      example.run
    ensure
      Rails.cache = original
    end

    it "re-enqueues the job (not silently discarded) when the breaker is open" do
      # Trip the breaker.
      Ai::CircuitBreaker::TRIP_THRESHOLD.times { Ai::CircuitBreaker.track_429!(provider: "mistral") }
      expect(Ai::CircuitBreaker.open?(provider: "mistral")).to be(true)

      # retry_on catches BackgroundBlocked and re-schedules the job.
      # ActiveJob's test adapter records re-enqueued jobs. We verify the job ends up
      # enqueued (a retry) rather than silently returning nil as the swallowed-error
      # path would. The fact that BackgroundBlocked propagates to the retry_on
      # handler (rather than being swallowed by a service-level rescue) is what
      # causes the re-enqueue; the exception hierarchy tests above prove the
      # propagation property directly.
      expect { BreakerTestBackgroundJob.perform_now }.to have_enqueued_job(BreakerTestBackgroundJob)
    end

    it "does NOT raise when the breaker is closed" do
      expect(Ai::CircuitBreaker.open?(provider: "mistral")).to be(false)
      expect { BreakerTestBackgroundJob.perform_now }.not_to raise_error
    end
  end

  # ── Interactive context passes through an open breaker ───────────────────────

  describe "interactive context bypasses the open breaker" do
    around do |example|
      original = Rails.cache
      Rails.cache = ActiveSupport::Cache::MemoryStore.new
      example.run
    ensure
      Rails.cache = original
      Thread.current[Ai::CircuitBreaker::INTERACTIVE_FLAG] = nil
    end

    it "does not raise for an as_interactive call even when the breaker is open" do
      Ai::CircuitBreaker::TRIP_THRESHOLD.times { Ai::CircuitBreaker.track_429!(provider: "mistral") }
      expect(Ai::CircuitBreaker.open?(provider: "mistral")).to be(true)

      expect do
        Ai::CircuitBreaker.as_interactive do
          Ai::CircuitBreaker.check!(provider: "mistral")
        end
      end.not_to raise_error
    end
  end
end
