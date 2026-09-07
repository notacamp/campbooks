# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::Budget do
  # Use a real in-memory cache (the test env uses :null_store which discards all
  # writes, so swap in MemoryStore for the duration of each example).
  around do |example|
    original = Rails.cache
    Rails.cache = ActiveSupport::Cache::MemoryStore.new
    example.run
  ensure
    Rails.cache = original
  end

  let(:workspace) { Workspace.create!(name: "Budget Test WS", plan: "pro") }

  # ── cap_for ─────────────────────────────────────────────────────────────────

  describe ".cap_for" do
    context "when self-hosted" do
      it "returns INFINITY regardless of adapter setup" do
        with_self_hosted do
          expect(described_class.cap_for(workspace)).to eq(Float::INFINITY)
        end
      end
    end

    context "when workspace is nil" do
      it "returns INFINITY" do
        expect(described_class.cap_for(nil)).to eq(Float::INFINITY)
      end
    end

    context "when workspace uses a BYO key (not managed)" do
      before do
        # Wire a BYO (non-managed) adapter for a text purpose
        adapter = workspace.ai_adapters.create!(name: "byo", provider: "openai", api_key: "sk-byo")
        workspace.ai_configurations.create!(
          ai_adapter: adapter, purpose: "global_chat", enabled: true,
          model: "gpt-4o-mini", max_tokens: 1000, temperature: 0.0
        )
      end

      it "returns INFINITY (BYO workspaces pay their own AI)" do
        expect(described_class.cap_for(workspace)).to eq(Float::INFINITY)
      end
    end

    context "when workspace uses managed AI" do
      before do
        adapter = workspace.ai_adapters.create!(name: "Campbooks AI — Text", provider: "mistral", managed: true)
        workspace.ai_configurations.create!(
          ai_adapter: adapter, purpose: "global_chat", enabled: true,
          model: "mistral-small-latest", max_tokens: 1000, temperature: 0.0
        )
      end

      it "returns the default ceiling when no ENV vars are set" do
        with_env("AI_DAILY_CALL_CEILING" => nil, "AI_PROBATION_DAILY_CAP" => nil) do
          expect(described_class.cap_for(workspace)).to eq(described_class::DEFAULT_DAILY_CEILING)
        end
      end

      it "returns the ENV ceiling when AI_DAILY_CALL_CEILING is set" do
        with_env("AI_DAILY_CALL_CEILING" => "500", "AI_PROBATION_DAILY_CAP" => nil) do
          expect(described_class.cap_for(workspace)).to eq(500)
        end
      end

      it "probation is off by default (AI_PROBATION_DAILY_CAP unset)" do
        # Even a brand-new workspace does not get probation cap unless the ENV is set
        with_env("AI_PROBATION_DAILY_CAP" => nil) do
          cap = described_class.cap_for(workspace)
          expect(cap).to eq(described_class::DEFAULT_DAILY_CEILING)
        end
      end

      context "when probation is enabled (AI_PROBATION_DAILY_CAP set)" do
        it "applies the probation cap for a workspace created within AI_PROBATION_DAYS" do
          # workspace.created_at is now (just created), well within 7 days
          with_env("AI_DAILY_CALL_CEILING" => "10000", "AI_PROBATION_DAILY_CAP" => "50",
                   "AI_PROBATION_DAYS" => "7") do
            expect(described_class.cap_for(workspace)).to eq(50)
          end
        end

        it "returns the full ceiling for a workspace created before the probation window" do
          # Travel back so workspace appears old
          workspace.update_column(:created_at, 10.days.ago)
          with_env("AI_DAILY_CALL_CEILING" => "10000", "AI_PROBATION_DAILY_CAP" => "50",
                   "AI_PROBATION_DAYS" => "7") do
            expect(described_class.cap_for(workspace)).to eq(10000)
          end
        end

        it "uses the lower of ceiling and probation cap" do
          with_env("AI_DAILY_CALL_CEILING" => "30", "AI_PROBATION_DAILY_CAP" => "50",
                   "AI_PROBATION_DAYS" => "7") do
            # Ceiling (30) is lower than probation cap (50) — ceiling wins
            expect(described_class.cap_for(workspace)).to eq(30)
          end
        end
      end
    end
  end

  # ── record! and exceeded? ────────────────────────────────────────────────────

  describe ".record! and .exceeded?" do
    before do
      # Managed adapter so cap_for returns a finite cap
      adapter = workspace.ai_adapters.create!(name: "Campbooks AI — Text", provider: "mistral", managed: true)
      workspace.ai_configurations.create!(
        ai_adapter: adapter, purpose: "global_chat", enabled: true,
        model: "mistral-small-latest", max_tokens: 1000, temperature: 0.0
      )
    end

    it "starts at zero — not exceeded" do
      with_env("AI_DAILY_CALL_CEILING" => "5", "AI_PROBATION_DAILY_CAP" => nil) do
        expect(described_class.exceeded?(workspace)).to be(false)
      end
    end

    it "increments on record! and eventually exceeds the cap" do
      with_env("AI_DAILY_CALL_CEILING" => "3", "AI_PROBATION_DAILY_CAP" => nil) do
        3.times { described_class.record!(workspace: workspace) }
        expect(described_class.exceeded?(workspace)).to be(true)
      end
    end

    it "does not exceed before the cap is reached" do
      with_env("AI_DAILY_CALL_CEILING" => "3", "AI_PROBATION_DAILY_CAP" => nil) do
        2.times { described_class.record!(workspace: workspace) }
        expect(described_class.exceeded?(workspace)).to be(false)
      end
    end

    it "never exceeds for nil workspace" do
      expect(described_class.exceeded?(nil)).to be(false)
    end

    it "never exceeds on self-hosted (INFINITY cap)" do
      with_self_hosted do
        100.times { described_class.record!(workspace: workspace) }
        expect(described_class.exceeded?(workspace)).to be(false)
      end
    end

    it "is resilient to cache errors in exceeded? (fail-open)" do
      allow(Rails.cache).to receive(:read).and_raise(StandardError, "cache broken")
      expect(described_class.exceeded?(workspace)).to be(false)
    end

    it "is resilient to cache errors in record! (no raise)" do
      allow(Rails.cache).to receive(:increment).and_raise(StandardError, "cache broken")
      expect { described_class.record!(workspace: workspace) }.not_to raise_error
    end
  end

  # ── enforce! ────────────────────────────────────────────────────────────────

  describe ".enforce!" do
    before do
      adapter = workspace.ai_adapters.create!(name: "Campbooks AI — Text", provider: "mistral", managed: true)
      workspace.ai_configurations.create!(
        ai_adapter: adapter, purpose: "global_chat", enabled: true,
        model: "mistral-small-latest", max_tokens: 1000, temperature: 0.0
      )
    end

    it "does not raise when under the cap" do
      with_env("AI_DAILY_CALL_CEILING" => "5", "AI_PROBATION_DAILY_CAP" => nil) do
        expect { described_class.enforce!(workspace: workspace) }.not_to raise_error
      end
    end

    it "raises Exceeded when the cap is reached" do
      with_env("AI_DAILY_CALL_CEILING" => "2", "AI_PROBATION_DAILY_CAP" => nil) do
        2.times { described_class.record!(workspace: workspace) }
        expect { described_class.enforce!(workspace: workspace) }.to \
          raise_error(described_class::Exceeded, /#{workspace.id}/)
      end
    end

    it "does not raise for nil workspace (no enforcement)" do
      expect { described_class.enforce!(workspace: nil) }.not_to raise_error
    end

    it "does not raise on self-hosted" do
      with_self_hosted do
        expect { described_class.enforce!(workspace: workspace) }.not_to raise_error
      end
    end
  end

  # ── Exceeded exception hierarchy ────────────────────────────────────────────

  describe "Exceeded" do
    it "inherits from Exception (not StandardError) so it bypasses rescue => e" do
      expect(described_class::Exceeded.ancestors).to include(Exception)
      expect(described_class::Exceeded.ancestors).not_to include(StandardError)
    end

    it "includes the workspace id in the message" do
      error = described_class::Exceeded.new(42)
      expect(error.message).to include("42")
    end
  end
end
