# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tools::QueryDocuments, type: :service do
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

  it "returns all documents when no filters are given" do
    doc = build_doc(vendor_name: "Acme")
    result = described_class.call({})
    expect(result[:documents].map { |d| d[:id] }).to include(doc.id)
    expect(result[:search_method]).to eq("text")
  end

  describe "vendor_name ILIKE filter (metadata)" do
    it "matches vendor_name stored in metadata" do
      match   = build_doc(vendor_name: "Umbrella Corp")
      no_match = build_doc(vendor_name: "Globex Inc")

      result = described_class.call("vendor_name" => "Umbrella")
      ids = result[:documents].map { |d| d[:id] }
      expect(ids).to include(match.id)
      expect(ids).not_to include(no_match.id)
    end

    it "does not raise on vendor_name with SQL-injection-style input" do
      build_doc(vendor_name: "Safe")
      expect { described_class.call("vendor_name" => "' OR '1'='1") }.not_to raise_error
    end
  end

  describe "amount_min_cents / amount_max_cents (guarded bigint cast)" do
    it "filters by amount_min_cents from metadata" do
      cheap  = build_doc(amount_cents: 500)
      pricey = build_doc(amount_cents: 20_000)

      result = described_class.call("amount_min_cents" => "1000")
      ids = result[:documents].map { |d| d[:id] }
      expect(ids).to include(pricey.id)
      expect(ids).not_to include(cheap.id)
    end

    it "filters by amount_max_cents from metadata" do
      cheap  = build_doc(amount_cents: 500)
      pricey = build_doc(amount_cents: 20_000)

      result = described_class.call("amount_max_cents" => "1000")
      ids = result[:documents].map { |d| d[:id] }
      expect(ids).to include(cheap.id)
      expect(ids).not_to include(pricey.id)
    end

    it "does not crash when a document has non-numeric amount_cents in metadata" do
      bad = build_doc(amount_cents: nil, tax_amount_cents: nil)
      bad.update_columns(metadata: (bad.metadata || {}).merge("amount_cents" => "not-a-number"))
      expect { described_class.call("amount_min_cents" => "100") }.not_to raise_error
    end
  end

  describe "date_from / date_to (metadata text comparison)" do
    it "filters documents on or after date_from" do
      after_date  = build_doc(document_date: Date.new(2026, 6, 15))
      before_date = build_doc(document_date: Date.new(2026, 5, 31))

      result = described_class.call("date_from" => "2026-06-01")
      ids = result[:documents].map { |d| d[:id] }
      expect(ids).to include(after_date.id)
      expect(ids).not_to include(before_date.id)
    end

    it "filters documents on or before date_to" do
      inside  = build_doc(document_date: Date.new(2026, 3, 15))
      outside = build_doc(document_date: Date.new(2026, 4, 1))

      result = described_class.call("date_to" => "2026-03-31")
      ids = result[:documents].map { |d| d[:id] }
      expect(ids).to include(inside.id)
      expect(ids).not_to include(outside.id)
    end
  end

  describe "ORDER BY document_date DESC NULLS LAST" do
    it "puts nil document_date docs last" do
      no_date = build_doc(document_date: nil, tax_amount_cents: nil)
      with_date = build_doc(document_date: Date.new(2026, 1, 1))

      result = described_class.call({})
      ids = result[:documents].map { |d| d[:id] }
      expect(ids.index(with_date.id)).to be < ids.index(no_date.id)
    end
  end

  it "fails closed when no workspace is established" do
    Current.workspace = nil
    result = described_class.call({})
    expect(result[:count]).to eq(0)
  end
end
