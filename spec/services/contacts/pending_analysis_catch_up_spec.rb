# frozen_string_literal: true

require "rails_helper"

RSpec.describe Contacts::PendingAnalysisCatchUp do
  let(:ws) { Workspace.create!(name: "Contact CatchUp WS") }

  def build_contact(email_count:, analyzed_at: nil, analysis_attempts: 0, sender_kind: :person)
    ws.contacts.create!(
      email: "c#{SecureRandom.hex(6)}@example.com",
      email_count: email_count,
      analyzed_at: analyzed_at,
      analysis_attempts: analysis_attempts,
      sender_kind: sender_kind
    )
  end

  before do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
  end

  it "enqueues analysis only for unanalyzed contacts past the threshold when a provider is configured" do
    build_contact(email_count: Contacts::Identifier::FIRST_ANALYSIS_THRESHOLD)
    build_contact(email_count: 40)
    build_contact(email_count: 40, analyzed_at: Time.current) # already analyzed — skip
    build_contact(email_count: 2)                             # below threshold — skip

    expect {
      described_class.run(ws)
    }.to have_enqueued_job(ContactAnalysisJob).exactly(2).times
  end

  it "enqueues nothing when no text provider is configured" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)

    build_contact(email_count: 10)
    expect {
      described_class.run(ws)
    }.not_to have_enqueued_job(ContactAnalysisJob)
  end

  it "caps each pass at LIMIT so a large backlog can't flood the queue" do
    (described_class::LIMIT + 3).times { build_contact(email_count: 5) }

    expect {
      described_class.run(ws)
    }.to have_enqueued_job(ContactAnalysisJob).exactly(described_class::LIMIT).times
  end

  it "is a safe no-op for a nil workspace" do
    expect {
      described_class.run(nil)
    }.not_to have_enqueued_job
  end

  # ── Analysis-attempts cap (guard against the forever-loop) ─────────────────

  it "excludes contacts that have reached MAX_ANALYSIS_ATTEMPTS" do
    capped = build_contact(email_count: 10, analysis_attempts: described_class::MAX_ANALYSIS_ATTEMPTS)
    over   = build_contact(email_count: 10, analysis_attempts: described_class::MAX_ANALYSIS_ATTEMPTS + 1)
    ok     = build_contact(email_count: 10, analysis_attempts: described_class::MAX_ANALYSIS_ATTEMPTS - 1)

    described_class.run(ws)

    expect(ContactAnalysisJob).to have_been_enqueued.with(ok.id)
    expect(ContactAnalysisJob).not_to have_been_enqueued.with(capped.id)
    expect(ContactAnalysisJob).not_to have_been_enqueued.with(over.id)
  end

  it "includes contacts with analysis_attempts below MAX_ANALYSIS_ATTEMPTS" do
    fresh = build_contact(email_count: 10, analysis_attempts: 0)
    one   = build_contact(email_count: 10, analysis_attempts: 1)

    described_class.run(ws)

    expect(ContactAnalysisJob).to have_been_enqueued.with(fresh.id)
    expect(ContactAnalysisJob).to have_been_enqueued.with(one.id)
  end

  # ── Sender-kind gate (exclude machine/service senders) ─────────────────────

  it "excludes service-kind contacts from the catch-up" do
    service = build_contact(email_count: 10, sender_kind: :service)
    person  = build_contact(email_count: 10, sender_kind: :person)

    described_class.run(ws)

    expect(ContactAnalysisJob).to have_been_enqueued.with(person.id)
    expect(ContactAnalysisJob).not_to have_been_enqueued.with(service.id)
  end

  it "includes person-kind contacts regardless of email volume" do
    high_volume_person = build_contact(email_count: 500, sender_kind: :person)

    expect {
      described_class.run(ws)
    }.to have_enqueued_job(ContactAnalysisJob).with(high_volume_person.id)
  end

  # ── Debounce (prevent a second immediate sweep from a web request) ──────────
  # The debounce gate uses Rails.cache with unless_exist: true, which is a no-op
  # in the null_store used by default in test. Use a real MemoryStore for these
  # examples (matches the pattern in standings_refresh_job_spec.rb).

  context "debounce" do
    around do |example|
      original_store = Rails.cache
      Rails.cache = ActiveSupport::Cache::MemoryStore.new
      example.run
    ensure
      Rails.cache = original_store
    end

    it "runs immediately when debounce is false (backfill job path)" do
      build_contact(email_count: 10)

      expect {
        described_class.run(ws, debounce: false)
      }.to have_enqueued_job(ContactAnalysisJob).exactly(1).times
    end

    it "runs the first call when debounce: true" do
      build_contact(email_count: 10)

      expect {
        described_class.run(ws, debounce: true)
      }.to have_enqueued_job(ContactAnalysisJob).exactly(1).times
    end

    it "skips a second immediate call within the debounce window" do
      build_contact(email_count: 10)

      described_class.run(ws, debounce: true)  # first — writes the gate

      expect {
        described_class.run(ws, debounce: true)  # second — gate already set, no-op
      }.not_to have_enqueued_job(ContactAnalysisJob)
    end

    it "allows a fresh call after the debounce window expires" do
      travel_to Time.utc(2026, 9, 7, 12, 0, 0) do
        build_contact(email_count: 10)
        described_class.run(ws, debounce: true)
      end

      travel_to Time.utc(2026, 9, 7, 12, 0, 0) + described_class::DEBOUNCE + 1.second do
        expect {
          described_class.run(ws, debounce: true)
        }.to have_enqueued_job(ContactAnalysisJob).exactly(1).times
      end
    end
  end
end
