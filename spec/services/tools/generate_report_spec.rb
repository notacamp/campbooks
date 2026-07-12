# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tools::GenerateReport, type: :service do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  before do
    Current.acting_user = user
    Current.workspace   = workspace
  end

  after { Current.reset }

  def build_doc(**attrs)
    doc = workspace.documents.new(
      document_type: "expense_invoice", source: :manual_upload,
      ai_status: :completed, review_status: :pending, **attrs
    )
    doc.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  describe "document_summary" do
    it "counts all workspace documents" do
      build_doc
      build_doc
      result = described_class.call("type" => "document_summary")
      expect(result[:total_documents]).to eq(2)
    end

    it "filters by date_from using metadata text comparison" do
      inside  = build_doc(document_date: Date.new(2026, 6, 15))
      outside = build_doc(document_date: Date.new(2026, 5, 31))

      result = described_class.call("type" => "document_summary", "date_from" => "2026-06-01")
      expect(result[:total_documents]).to eq(1)
      expect(result[:total_documents]).not_to eq(2) # outside doc excluded
    end

    it "filters by date_to using metadata text comparison" do
      inside  = build_doc(document_date: Date.new(2026, 3, 10))
      outside = build_doc(document_date: Date.new(2026, 4, 1))

      result = described_class.call("type" => "document_summary", "date_to" => "2026-03-31")
      expect(result[:total_documents]).to eq(1)
    end

    it "sums amount_cents via guarded bigint cast (does not raise on junk metadata)" do
      build_doc(amount_cents: 10_000)
      build_doc(amount_cents: 5_000)
      junk = build_doc(amount_cents: nil, tax_amount_cents: nil)
      junk.update_columns(metadata: (junk.metadata || {}).merge("amount_cents" => "not-a-number"))

      result = described_class.call("type" => "document_summary")
      expect(result[:total_amount_cents]).to eq(15_000)
    end

    it "returns 0 for total_amount_cents when all amounts are nil" do
      build_doc(amount_cents: nil, tax_amount_cents: nil)
      result = described_class.call("type" => "document_summary")
      expect(result[:total_amount_cents]).to eq(0)
    end
  end
end
