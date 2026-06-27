require "rails_helper"
RSpec.describe DocumentTemplates::PdfGenerator do
  let(:h) { "<html><body>T</body></html>" }
  it "calls Grover" do
    skip "Grover not available" unless defined?(Grover)
    g = instance_double(Grover, to_pdf: "PDF")
    expect(Grover).to receive(:new).with(h,hash_including(format:"A4")).and_return(g)
    expect(described_class.call(h)).to eq("PDF")
  end
  it "raises on failure" do
    pending "Grover not available" unless defined?(Grover)
    allow(Grover).to receive(:new).and_raise(StandardError.new("e"))
    expect{described_class.call(h)}.to raise_error(DocumentTemplates::PdfGenerator::PdfGenerationError)
  end
end
