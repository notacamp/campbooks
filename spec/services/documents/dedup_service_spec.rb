# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::DedupService do
  describe "merge_into — metadata merge policy" do
    it "adopts dup's full metadata and AI data when keep has none" do
      keep = create(:document,
                    metadata: { "vendor_name" => nil },
                    ai_extraction_data: nil,
                    ai_confidence_score: nil)
      dup  = create(:document,
                    metadata: { "vendor_name" => "Dup Vendor", "invoice_number" => "INV-1",
                                "amount_cents" => 5000 },
                    ai_extraction_data: { "source" => "dup" },
                    ai_confidence_score: 0.85)

      described_class.new(dry_run: false).send(:merge_into, keep.id, dup.id)

      keep.reload
      expect(keep.vendor_name).to eq("Dup Vendor")
      expect(keep.invoice_number).to eq("INV-1")
      expect(keep.amount_cents).to eq(5000)
      expect(keep.ai_extraction_data).to eq({ "source" => "dup" })
      expect(keep.ai_confidence_score).to eq(0.85)
    end

    it "dup's non-blank fields win over keep's for specific fields (dup has the better AI data)" do
      keep = create(:document,
                    metadata: { "vendor_name" => "Keep Vendor" },
                    ai_extraction_data: nil)
      dup  = create(:document,
                    metadata: { "vendor_name" => "Dup Vendor", "bank_name" => "Dup Bank" },
                    ai_extraction_data: { "x" => "y" })

      described_class.new(dry_run: false).send(:merge_into, keep.id, dup.id)

      keep.reload
      # dup's vendor_name wins (dup has the AI extraction data)
      expect(keep.vendor_name).to eq("Dup Vendor")
      # dup's bank_name carried over
      expect(keep.bank_name).to eq("Dup Bank")
    end

    it "falls back to keep's value for specific fields when dup's is blank" do
      keep = create(:document,
                    metadata: { "vendor_name" => "Keep Vendor", "invoice_number" => "INV-KEEP" },
                    ai_extraction_data: nil)
      dup  = create(:document,
                    metadata: { "vendor_name" => nil, "invoice_number" => nil },
                    ai_extraction_data: { "x" => "y" })

      described_class.new(dry_run: false).send(:merge_into, keep.id, dup.id)

      keep.reload
      # dup's vendor_name is blank → keep's is the fallback
      expect(keep.vendor_name).to eq("Keep Vendor")
      expect(keep.invoice_number).to eq("INV-KEEP")
    end

    it "does not touch keep when keep already has AI extraction data" do
      keep = create(:document,
                    metadata: { "vendor_name" => "Keep Vendor" },
                    ai_extraction_data: { "keep" => "data" })
      dup  = create(:document,
                    metadata: { "vendor_name" => "Dup Vendor" },
                    ai_extraction_data: { "dup" => "data" })

      described_class.new(dry_run: false).send(:merge_into, keep.id, dup.id)

      keep.reload
      expect(keep.vendor_name).to eq("Keep Vendor")
      expect(keep.ai_extraction_data).to eq({ "keep" => "data" })
    end

    it "is a no-op in dry_run mode" do
      keep = create(:document, ai_extraction_data: nil, metadata: {})
      dup  = create(:document, ai_extraction_data: { "x" => "y" },
                    metadata: { "vendor_name" => "Dry Vendor" })

      described_class.new(dry_run: true).send(:merge_into, keep.id, dup.id)

      keep.reload
      expect(keep.vendor_name).to be_nil
      expect(Document.find_by(id: dup.id)).to be_present
    end
  end
end
