# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::CircuitBreaker do
  # Use a real in-memory cache for these tests so cache semantics hold without
  # persisting state between the suite runs. The test env uses :null_store, which
  # discards all writes, so we swap in MemoryStore for the duration of the spec.
  around do |example|
    original = Rails.cache
    Rails.cache = ActiveSupport::Cache::MemoryStore.new
    example.run
  ensure
    Rails.cache = original
    Thread.current[described_class::INTERACTIVE_FLAG] = nil
  end

  # ── interactive? ──────────────────────────────────────────────────────────────

  describe ".interactive?" do
    it "returns false by default (background is the safe default)" do
      expect(described_class.interactive?).to be(false)
    end

    it "returns true inside as_interactive" do
      described_class.as_interactive do
        expect(described_class.interactive?).to be(true)
      end
    end

    it "restores the flag to its original value after the block" do
      expect(described_class.interactive?).to be(false)
      described_class.as_interactive { nil }
      expect(described_class.interactive?).to be(false)
    end

    it "restores the flag even if the block raises" do
      expect do
        described_class.as_interactive { raise "oops" }
      end.to raise_error("oops")

      expect(described_class.interactive?).to be(false)
    end
  end

  # ── open? ─────────────────────────────────────────────────────────────────────

  describe ".open?" do
    it "returns false when no breaker data exists in cache" do
      expect(described_class.open?(provider: "mistral")).to be(false)
    end

    it "returns true when the open flag is in cache" do
      Rails.cache.write("ai:cb:open:mistral", true, expires_in: 120.seconds)
      expect(described_class.open?(provider: "mistral")).to be(true)
    end

    it "returns false when the cache store raises (fail-open behaviour)" do
      allow(Rails.cache).to receive(:read).and_raise(StandardError, "cache broken")
      expect(described_class.open?(provider: "mistral")).to be(false)
    end
  end

  # ── track_429! ────────────────────────────────────────────────────────────────

  describe ".track_429!" do
    it "increments the counter on each call" do
      described_class.track_429!(provider: "mistral")
      described_class.track_429!(provider: "mistral")
      count = Rails.cache.read("ai:cb:count:mistral").to_i
      expect(count).to eq(2)
    end

    it "does not open the breaker before TRIP_THRESHOLD is reached" do
      (described_class::TRIP_THRESHOLD - 1).times do
        described_class.track_429!(provider: "mistral")
      end
      expect(described_class.open?(provider: "mistral")).to be(false)
    end

    it "opens the breaker when TRIP_THRESHOLD is reached" do
      described_class::TRIP_THRESHOLD.times do
        described_class.track_429!(provider: "mistral")
      end
      expect(described_class.open?(provider: "mistral")).to be(true)
    end

    it "isolates counts per provider" do
      described_class::TRIP_THRESHOLD.times { described_class.track_429!(provider: "mistral") }
      expect(described_class.open?(provider: "openai")).to be(false)
    end

    it "is resilient to cache errors (does not raise)" do
      allow(Rails.cache).to receive(:read).and_raise(StandardError, "cache broken")
      allow(Rails.cache).to receive(:write).and_raise(StandardError, "cache broken")
      expect { described_class.track_429!(provider: "mistral") }.not_to raise_error
    end
  end

  # ── check! ────────────────────────────────────────────────────────────────────

  describe ".check!" do
    context "when the breaker is closed" do
      it "does not raise for background callers" do
        expect { described_class.check!(provider: "mistral") }.not_to raise_error
      end

      it "does not raise for interactive callers" do
        described_class.as_interactive do
          expect { described_class.check!(provider: "mistral") }.not_to raise_error
        end
      end
    end

    context "when the breaker is open" do
      before do
        described_class::TRIP_THRESHOLD.times { described_class.track_429!(provider: "mistral") }
      end

      it "raises BackgroundBlocked for background callers" do
        expect { described_class.check!(provider: "mistral") }.to \
          raise_error(described_class::BackgroundBlocked, /mistral/)
      end

      it "does not raise for interactive callers (compose/Scout bypass)" do
        described_class.as_interactive do
          expect { described_class.check!(provider: "mistral") }.not_to raise_error
        end
      end
    end

    context "when the cache raises" do
      it "fails open — does not block callers" do
        allow(Rails.cache).to receive(:read).and_raise(StandardError, "cache broken")
        expect { described_class.check!(provider: "mistral") }.not_to raise_error
      end
    end
  end
end
