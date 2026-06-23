require "rails_helper"

RSpec.describe Documents::FilenameGenerator do
  describe "#call" do
    it "generates filename for expense invoices" do
      document = build(:document,
        document_type: :expense_invoice,
        vendor_name: "Empresa Exemplo Lda",
        document_date: Date.new(2025, 1, 15),
        invoice_number: "FT2025/0042")

      filename = described_class.new(document).call
      expect(filename).to eq("empresa_exemplo_lda_20250115_ft2025_0042.pdf")
    end

    it "generates filename for revenue invoices" do
      document = build(:document, :revenue_invoice,
        client_name: "Client Corp",
        document_date: Date.new(2025, 3, 10),
        invoice_number: "FT2025/0100")

      filename = described_class.new(document).call
      expect(filename).to eq("client_corp_20250310_ft2025_0100.pdf")
    end

    it "generates filename for bank statements" do
      document = build(:document, :bank_statement,
        bank_name: "Millennium BCP",
        period_start: Date.new(2025, 1, 1),
        period_end: Date.new(2025, 1, 31))

      filename = described_class.new(document).call
      expect(filename).to eq("millennium_bcp_20250101_to_20250131.pdf")
    end

    it "generates filename for receipts" do
      document = build(:document, :receipt,
        vendor_name: "Loja ABC",
        document_date: Date.new(2025, 2, 20),
        receipt_number: "RC2025/0055")

      filename = described_class.new(document).call
      expect(filename).to eq("loja_abc_20250220_rc2025_0055.pdf")
    end

    it "generates filename for other documents" do
      document = build(:document, :other,
        vendor_name: "Some Entity",
        document_date: Date.new(2025, 4, 5))

      filename = described_class.new(document).call
      expect(filename).to eq("some_entity_20250405.pdf")
    end

    it "handles missing fields gracefully" do
      document = build(:document, id: 1, vendor_name: nil, document_date: nil, invoice_number: nil)
      filename = described_class.new(document).call
      expect(filename).to match(/document_.*\.pdf/)
    end

    it "sanitizes special characters" do
      document = build(:document,
        vendor_name: "Café & Restau/ração",
        document_date: Date.new(2025, 3, 1),
        invoice_number: "FT001")

      filename = described_class.new(document).call
      expect(filename).not_to include("&")
      expect(filename).not_to include("/")
      expect(filename).to end_with(".pdf")
    end
  end
end
