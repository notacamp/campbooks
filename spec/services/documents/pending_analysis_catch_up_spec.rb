# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::PendingAnalysisCatchUp do
  include ActiveJob::TestHelper

  before { @ws = Workspace.create!(name: "CatchUp WS") }

  def build_doc(ai_status)
    doc = @ws.documents.new(document_type: "other", ai_status: ai_status,
                            review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  it "enqueues analysis only for ai_pending docs when a provider is configured" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
    build_doc(:pending)
    build_doc(:pending)
    build_doc(:completed) # already analyzed — must be skipped

    assert_enqueued_jobs(2, only: DocumentProcessJob) do
      described_class.run(@ws)
    end
  end

  it "enqueues nothing when no document provider is configured" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)
    build_doc(:pending)
    assert_no_enqueued_jobs { described_class.run(@ws) }
  end

  it "caps each pass at LIMIT so a large backlog can't flood the queue" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
    (described_class::LIMIT + 3).times { build_doc(:pending) }

    assert_enqueued_jobs(described_class::LIMIT, only: DocumentProcessJob) do
      described_class.run(@ws)
    end
  end

  it "is a safe no-op for a nil workspace" do
    assert_no_enqueued_jobs { described_class.run(nil) }
  end
end
