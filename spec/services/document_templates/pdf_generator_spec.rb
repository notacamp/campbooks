require "rails_helper"

RSpec.describe DocumentTemplates::PdfGenerator do
  let(:html) { "<html><body>Hello</body></html>" }

  it "renders HTML to a PDF byte string via Grover" do
    grover = instance_double(Grover, to_pdf: "%PDF-1.4")
    expect(Grover).to receive(:new).with(html, hash_including(format: "A4")).and_return(grover)

    expect(described_class.call(html)).to eq("%PDF-1.4")
  end

  it "wraps a browser failure in PdfGenerationError" do
    allow(Grover).to receive(:new).and_raise(StandardError.new("Failed to launch the browser process"))

    expect { described_class.call(html) }
      .to raise_error(DocumentTemplates::PdfGenerator::PdfGenerationError, /browser/)
  end

  it "raises PdfGenerationError for blank input without invoking Grover" do
    expect(Grover).not_to receive(:new)

    expect { described_class.call("") }
      .to raise_error(DocumentTemplates::PdfGenerator::PdfGenerationError)
  end
end
