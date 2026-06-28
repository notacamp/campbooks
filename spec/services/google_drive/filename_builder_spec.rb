require "rails_helper"

RSpec.describe GoogleDrive::FilenameBuilder do
  let(:document_type) { build_stubbed(:document_type, name: "expense_invoice") }
  let(:config) { build_stubbed(:google_drive_config, document_type: document_type, naming_pattern: pattern) }
  let(:pattern) { "{date}_{entity}_{reference}" }
  let(:document) do
    build_stubbed(:document,
      document_type: "expense_invoice",
      document_date: Date.new(2026, 6, 27),
      classification: document_type
    )
  end

  before do
    allow(document).to receive(:entity_display_name).and_return("ACME Corp")
    allow(document).to receive(:reference_display).and_return("INV-2026-0042")
    allow(document).to receive(:vendor_name).and_return("ACME Corp")
  end

  describe "#call" do
    it "replaces {date} with YYYYMMDD" do
      expect(described_class.new(document, config).call).to include("20260627")
    end

    it "replaces {entity} with sanitized name" do
      expect(described_class.new(document, config).call).to include("ACME_Corp")
    end

    it "replaces {reference} with sanitized reference" do
      expect(described_class.new(document, config).call).to include("INV-2026-0042")
    end

    it "replaces {type} with document type name" do
      c = build_stubbed(:google_drive_config, document_type: document_type, naming_pattern: "{type}")
      expect(described_class.new(document, c).call).to eq("expense_invoice")
    end

    it "replaces {id} with document id" do
      allow(document).to receive(:id).and_return(42)
      c = build_stubbed(:google_drive_config, document_type: document_type, naming_pattern: "{id}")
      expect(described_class.new(document, c).call).to eq("42")
    end

    it "falls back to today when document_date is nil" do
      allow(document).to receive(:document_date).and_return(nil)
      c = build_stubbed(:google_drive_config, document_type: document_type, naming_pattern: "{date}")
      expect(described_class.new(document, c).call).to eq(Date.current.strftime("%Y%m%d"))
    end

    it "uses 'unknown' when entity is blank" do
      allow(document).to receive(:entity_display_name).and_return(nil)
      c = build_stubbed(:google_drive_config, document_type: document_type, naming_pattern: "{entity}")
      expect(described_class.new(document, c).call).to eq("unknown")
    end

    it "collapses consecutive underscores from special chars" do
      allow(document).to receive(:entity_display_name).and_return("ACME Corp / Subsidiary")
      c = build_stubbed(:google_drive_config, document_type: document_type, naming_pattern: "{entity}")
      expect(described_class.new(document, c).call).to eq("ACME_Corp_Subsidiary")
    end

    it "produces the default pattern" do
      expect(described_class.new(document, config).call).to eq("20260627_ACME_Corp_INV-2026-0042")
    end
  end
end
