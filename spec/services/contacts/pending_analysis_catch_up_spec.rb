# frozen_string_literal: true

require "rails_helper"

RSpec.describe Contacts::PendingAnalysisCatchUp do
  let(:ws) { Workspace.create!(name: "Contact CatchUp WS") }

  def build_contact(email_count:, analyzed_at: nil)
    ws.contacts.create!(
      email: "c#{SecureRandom.hex(6)}@example.com",
      email_count: email_count,
      analyzed_at: analyzed_at
    )
  end

  it "enqueues analysis only for unanalyzed contacts past the threshold when a provider is configured" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)

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
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)

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
end
