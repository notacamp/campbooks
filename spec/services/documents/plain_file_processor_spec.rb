require "rails_helper"

RSpec.describe Documents::PlainFileProcessor do
  let(:workspace) { create(:workspace) }
  let(:document) do
    doc = create(:document, workspace: workspace, ai_status: :pending)
    doc.original_file.attach(io: StringIO.new("BEGIN:VCALENDAR"), filename: "invite.ics", content_type: "text/calendar")
    doc
  end

  it "marks the document ai_skipped without ever invoking the AI analyzer" do
    expect(Ai::DocumentAnalyzer).not_to receive(:new)

    described_class.new(document).call

    expect(document.reload).to be_ai_skipped
  end

  it "publishes a document.processed event" do
    allow(Events).to receive(:publish)

    described_class.new(document).call

    expect(Events).to have_received(:publish).with("document.processed", hash_including(subject: document))
  end

  it "records the error and re-raises when processing fails" do
    allow(Documents::TitleGenerator).to receive(:new).and_raise(RuntimeError, "boom")

    expect { described_class.new(document).call }.to raise_error(RuntimeError, "boom")
    expect(document.reload).to be_ai_failed
    expect(document.ai_error).to eq("boom")
  end
end
