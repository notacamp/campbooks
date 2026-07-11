require "rails_helper"

RSpec.describe Documents::Processor do
  let(:workspace) { create(:workspace) }

  # The factory attaches a pdf; re-attach to exercise each content type.
  def document_with(content_type, filename)
    doc = create(:document, workspace: workspace)
    doc.original_file.attach(io: StringIO.new("x"), filename: filename, content_type: content_type)
    doc
  end

  it "routes an analyzable file (pdf) to AnalyzableProcessor" do
    document = document_with("application/pdf", "invoice.pdf")
    sub = instance_double(Documents::AnalyzableProcessor, call: document)
    expect(Documents::AnalyzableProcessor).to receive(:new).with(document).and_return(sub)
    expect(Documents::PlainFileProcessor).not_to receive(:new)

    expect(described_class.new(document).call).to eq(document)
  end

  it "routes a non-document (ics) to PlainFileProcessor" do
    document = document_with("text/calendar", "invite.ics")
    sub = instance_double(Documents::PlainFileProcessor, call: document)
    expect(Documents::PlainFileProcessor).to receive(:new).with(document).and_return(sub)
    expect(Documents::AnalyzableProcessor).not_to receive(:new)

    expect(described_class.new(document).call).to eq(document)
  end

  it "treats archives, raw emails, and html as non-documents" do
    %w[application/zip message/rfc822 text/html text/calendar].each do |content_type|
      expect(document_with(content_type, "file").analyzable?).to be(false)
    end
  end

  it "treats pdf, image, and unknown types as analyzable" do
    expect(document_with("application/pdf", "a.pdf").analyzable?).to be(true)
    expect(document_with("image/png", "a.png").analyzable?).to be(true)
    expect(document_with("application/octet-stream", "a.bin").analyzable?).to be(true)
  end
end
