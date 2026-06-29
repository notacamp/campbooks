require "rails_helper"

RSpec.describe EmailTemplates::Applier do
  let(:ws) { create(:workspace) }
  let(:u) { create(:user, workspace: ws) }
  let(:template) { create(:email_template, :ai_completed, :with_documents, workspace: ws) }

  before { allow(DocumentTemplates::PdfGenerator).to receive(:call).and_return("PDF-BYTES") }

  describe ".call" do
    it "renders subject and body with the variables" do
      result = described_class.call(template: template, variables: { "recipient_name" => "Ada", "workspace_name" => "Acme" }, user: u)
      expect(result.subject).to eq("Welcome, Ada!")
      expect(result.body_html).to include("Hi Ada, welcome to Acme.")
    end

    it "uploads a PDF attachment per document template and returns signed ids" do
      result = described_class.call(template: template, variables: {}, user: u)
      expect(result.attachments.size).to eq(1)
      expect(result.attachments.first[:signed_id]).to be_present
      expect(result.attachments.first[:content_type]).to eq("application/pdf")
      expect(u.outbound_attachments).to be_attached
    end
  end

  describe ".pdf_attachments" do
    it "returns ready-to-send data hashes without persisting blobs" do
      atts = described_class.pdf_attachments(template: template, variables: {})
      expect(atts.size).to eq(1)
      expect(atts.first[:data]).to eq("PDF-BYTES")
      expect(atts.first[:filename]).to end_with(".pdf")
    end

    it "skips a document template whose PDF rendering fails" do
      allow(DocumentTemplates::PdfGenerator).to receive(:call).and_raise(StandardError, "boom")
      expect(described_class.pdf_attachments(template: template, variables: {})).to eq([])
    end
  end
end
