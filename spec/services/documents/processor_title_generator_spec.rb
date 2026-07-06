# frozen_string_literal: true

require "rails_helper"

# Tests that Documents::Processor invokes TitleGenerator after a successful AI
# analysis pass, and that no email-address titled docs slip through.
RSpec.describe "Documents::Processor + TitleGenerator integration" do
  before do
    @ws = Workspace.create!(name: "Processor TG WS")
  end

  def build_doc(sender_name: nil, metadata: nil)
    doc = @ws.documents.new(
      document_type: "other",
      ai_status: :pending,
      review_status: :pending,
      source: :email,
      sender_name: sender_name,
      metadata: metadata
    )
    doc.original_file.attach(io: StringIO.new("data"), filename: "test.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  it "Processor calls TitleGenerator after a successful AI pass" do
    doc = build_doc

    allow_any_instance_of(Ai::DocumentAnalyzer).to receive(:call) do
      doc.update_columns(ai_status: Document.ai_statuses[:completed],
                         metadata: (doc.metadata || {}).merge("title" => "Vendor Invoice — Jan 2026"))
      { error: nil }
    end

    calls = []
    allow_any_instance_of(Documents::TitleGenerator).to receive(:call) do
      calls << doc.id
      nil
    end

    Documents::Processor.new(doc).call
    expect(calls).to include(doc.id)
  end

  it "Processor does NOT call TitleGenerator when AI fails (returns early)" do
    doc = build_doc

    allow_any_instance_of(Ai::DocumentAnalyzer).to receive(:call) do
      doc.update_columns(ai_status: Document.ai_statuses[:failed], ai_error: "timeout")
      { error: "timeout" }
    end

    calls = []
    allow_any_instance_of(Documents::TitleGenerator).to receive(:call) do
      calls << doc.id
      nil
    end

    Documents::Processor.new(doc).call
    expect(calls).to be_empty
  end
end
